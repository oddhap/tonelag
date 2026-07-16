use std::{
    ffi::{CString, c_void},
    ptr, slice,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, sync_channel},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use ffmpeg::{ffi, format};
use ffmpeg_next as ffmpeg;
use futures_util::StreamExt;
use tokio::sync::mpsc as tokio_mpsc;

use crate::audio::AudioEvent;

const AVIO_BUFFER_SIZE: usize = 32 * 1024;

pub enum InputOwner {
    File(format::context::Input),
    Http(HttpInput),
}

impl InputOwner {
    pub fn context_mut(&mut self) -> &mut format::context::Input {
        match self {
            Self::File(context) => context,
            Self::Http(input) => &mut input.context,
        }
    }
}

pub struct HttpInput {
    // Keep this order: avformat must close before its custom `pb` is released.
    context: format::context::Input,
    _io: CustomIo,
}

enum NetworkChunk {
    Data(Vec<u8>),
    Eof,
    Error(String),
}

struct IcyReader {
    chunks: Receiver<NetworkChunk>,
    current: Vec<u8>,
    current_offset: usize,
    metadata_interval: Option<usize>,
    audio_until_metadata: usize,
    events: tokio_mpsc::UnboundedSender<AudioEvent>,
    last_title: String,
    external_cancel: Arc<AtomicBool>,
    local_cancel: Arc<AtomicBool>,
}

impl IcyReader {
    fn new(
        chunks: Receiver<NetworkChunk>,
        metadata_interval: Option<usize>,
        events: tokio_mpsc::UnboundedSender<AudioEvent>,
        external_cancel: Arc<AtomicBool>,
        local_cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            chunks,
            current: Vec::new(),
            current_offset: 0,
            metadata_interval,
            audio_until_metadata: metadata_interval.unwrap_or(usize::MAX),
            events,
            last_title: String::new(),
            external_cancel,
            local_cancel,
        }
    }

    fn read_audio(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let mut written = 0;
        while written < output.len() {
            if self.metadata_interval.is_some() && self.audio_until_metadata == 0 {
                self.read_metadata()?;
                self.audio_until_metadata = self.metadata_interval.unwrap_or(usize::MAX);
            }
            let count = (output.len() - written).min(self.audio_until_metadata);
            let read = self.read_raw(&mut output[written..written + count])?;
            if read == 0 {
                break;
            }
            written += read;
            self.audio_until_metadata = self.audio_until_metadata.saturating_sub(read);
        }
        Ok(written)
    }

    fn read_metadata(&mut self) -> std::io::Result<()> {
        let mut length = [0_u8; 1];
        self.read_raw_exact(&mut length)?;
        let byte_count = usize::from(length[0]) * 16;
        if byte_count == 0 {
            return Ok(());
        }
        let mut metadata = vec![0_u8; byte_count];
        self.read_raw_exact(&mut metadata)?;
        let metadata = String::from_utf8_lossy(&metadata);
        let title = extract_stream_title(&metadata).unwrap_or_default();
        if !title.is_empty() && title != self.last_title {
            self.last_title = title.to_owned();
            let _ = self
                .events
                .send(AudioEvent::Metadata(self.last_title.clone()));
        }
        Ok(())
    }

    fn read_raw(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.external_cancel.load(Ordering::Acquire)
                || self.local_cancel.load(Ordering::Acquire)
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "stream cancelled",
                ));
            }
            if self.current_offset < self.current.len() {
                let count = output.len().min(self.current.len() - self.current_offset);
                output[..count].copy_from_slice(
                    &self.current[self.current_offset..self.current_offset + count],
                );
                self.current_offset += count;
                return Ok(count);
            }
            match self.chunks.recv_timeout(Duration::from_millis(50)) {
                Ok(NetworkChunk::Data(bytes)) => {
                    self.current = bytes;
                    self.current_offset = 0;
                }
                Ok(NetworkChunk::Eof) => return Ok(0),
                Ok(NetworkChunk::Error(message)) => {
                    return Err(std::io::Error::other(message));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
    }

    fn read_raw_exact(&mut self, output: &mut [u8]) -> std::io::Result<()> {
        let mut offset = 0;
        while offset < output.len() {
            let count = self.read_raw(&mut output[offset..])?;
            if count == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "short ICY metadata block",
                ));
            }
            offset += count;
        }
        Ok(())
    }
}

impl Drop for IcyReader {
    fn drop(&mut self) {
        self.local_cancel.store(true, Ordering::Release);
    }
}

struct CustomIo {
    context: *mut ffi::AVIOContext,
    reader: *mut IcyReader,
}

unsafe impl Send for CustomIo {}

impl Drop for CustomIo {
    fn drop(&mut self) {
        // SAFETY: both allocations are created in `open_http` and uniquely owned here.
        unsafe {
            if !self.context.is_null() {
                ffi::av_free((*self.context).buffer.cast::<c_void>());
                (*self.context).buffer = ptr::null_mut();
                ffi::avio_context_free(&mut self.context);
            }
            if !self.reader.is_null() {
                drop(Box::from_raw(self.reader));
                self.reader = ptr::null_mut();
            }
        }
    }
}

pub fn open_file(path: &str) -> Result<InputOwner> {
    Ok(InputOwner::File(
        format::input(path).with_context(|| format!("failed opening {path}"))?,
    ))
}

pub fn open_http(
    url: &str,
    events: tokio_mpsc::UnboundedSender<AudioEvent>,
    external_cancel: Arc<AtomicBool>,
) -> Result<InputOwner> {
    let safe_url = crate::privacy::redact_url(url);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("Tonelag/0.1")
        .build()?;
    let request = client.get(url).header("Icy-MetaData", "1").send();
    let response = runtime.block_on(async {
        tokio::pin!(request);
        loop {
            tokio::select! {
                response = &mut request => {
                    break response.map_err(|error| {
                        anyhow!("request to {safe_url} failed: {}", error.without_url())
                    });
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if external_cancel.load(Ordering::Acquire) {
                        break Err(anyhow!("stream request cancelled"));
                    }
                }
            }
        }
    })?;
    let response = response.error_for_status().map_err(|error| {
        anyhow!(
            "request to {safe_url} returned {}",
            error.status().map(|status| status.as_u16()).unwrap_or(0)
        )
    })?;
    let metadata_interval = response
        .headers()
        .get("icy-metaint")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0);

    let (chunk_tx, chunk_rx) = sync_channel(8);
    let local_cancel = Arc::new(AtomicBool::new(false));
    let download_local_cancel = local_cancel.clone();
    let download_external_cancel = external_cancel.clone();
    std::thread::Builder::new()
        .name("classic-http-stream".into())
        .spawn(move || {
            runtime.block_on(async move {
                let mut body = response.bytes_stream();
                loop {
                    tokio::select! {
                        chunk = body.next() => match chunk {
                            Some(Ok(bytes)) => {
                                if chunk_tx.send(NetworkChunk::Data(bytes.to_vec())).is_err() {
                                    break;
                                }
                            }
                            Some(Err(error)) => {
                                let _ = chunk_tx.send(NetworkChunk::Error(error.without_url().to_string()));
                                break;
                            }
                            None => {
                                let _ = chunk_tx.send(NetworkChunk::Eof);
                                break;
                            }
                        },
                        _ = tokio::time::sleep(Duration::from_millis(50)) => {
                            if download_local_cancel.load(Ordering::Acquire)
                                || download_external_cancel.load(Ordering::Acquire)
                            {
                                break;
                            }
                        }
                    }
                }
            });
        })?;

    let reader = Box::into_raw(Box::new(IcyReader::new(
        chunk_rx,
        metadata_interval,
        events,
        external_cancel,
        local_cancel,
    )));
    allocate_ffmpeg_input(url, &safe_url, reader)
}

fn allocate_ffmpeg_input(url: &str, safe_url: &str, reader: *mut IcyReader) -> Result<InputOwner> {
    // SAFETY: allocations and FFmpeg pointers are checked and transferred to RAII owners.
    unsafe {
        let buffer = ffi::av_malloc(AVIO_BUFFER_SIZE).cast::<u8>();
        if buffer.is_null() {
            drop(Box::from_raw(reader));
            bail!("FFmpeg could not allocate an AVIO buffer");
        }
        let avio = ffi::avio_alloc_context(
            buffer,
            AVIO_BUFFER_SIZE as i32,
            0,
            reader.cast::<c_void>(),
            Some(read_packet),
            None,
            None,
        );
        if avio.is_null() {
            ffi::av_free(buffer.cast::<c_void>());
            drop(Box::from_raw(reader));
            bail!("FFmpeg could not allocate an AVIO context");
        }
        let custom = CustomIo {
            context: avio,
            reader,
        };
        let mut format_context = ffi::avformat_alloc_context();
        if format_context.is_null() {
            bail!("FFmpeg could not allocate an input context");
        }
        (*format_context).pb = avio;
        (*format_context).flags |= ffi::AVFMT_FLAG_CUSTOM_IO;
        let name = CString::new(url).context("stream URL contains a NUL byte")?;
        let result = ffi::avformat_open_input(
            &mut format_context,
            name.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if result < 0 {
            if !format_context.is_null() {
                ffi::avformat_free_context(format_context);
            }
            bail!(
                "FFmpeg could not probe {safe_url}: {}",
                ffmpeg::Error::from(result)
            );
        }
        let result = ffi::avformat_find_stream_info(format_context, ptr::null_mut());
        if result < 0 {
            ffi::avformat_close_input(&mut format_context);
            bail!(
                "FFmpeg could not read stream information: {}",
                ffmpeg::Error::from(result)
            );
        }
        Ok(InputOwner::Http(HttpInput {
            context: format::context::Input::wrap(format_context),
            _io: custom,
        }))
    }
}

unsafe extern "C" fn read_packet(opaque: *mut c_void, buffer: *mut u8, size: i32) -> i32 {
    if opaque.is_null() || buffer.is_null() || size <= 0 {
        return ffi::AVERROR_EXTERNAL;
    }
    std::panic::catch_unwind(|| {
        // SAFETY: FFmpeg passes the pointers originally supplied to `avio_alloc_context`.
        let reader = unsafe { &mut *opaque.cast::<IcyReader>() };
        let output = unsafe { slice::from_raw_parts_mut(buffer, size as usize) };
        match reader.read_audio(output) {
            Ok(0) => ffi::AVERROR_EOF,
            Ok(count) => count as i32,
            Err(_) => ffi::AVERROR_EXTERNAL,
        }
    })
    .unwrap_or(ffi::AVERROR_EXTERNAL)
}

fn extract_stream_title(metadata: &str) -> Option<&str> {
    metadata
        .split(';')
        .find_map(|field| field.trim().strip_prefix("StreamTitle='"))
        .and_then(|value| value.strip_suffix('\''))
        .map(|value| value.trim_matches('\0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_icy_stream_title() {
        assert_eq!(
            extract_stream_title("StreamTitle='Artist - Track';StreamUrl='';"),
            Some("Artist - Track")
        );
    }

    #[test]
    fn strips_icy_metadata_from_audio_bytes() {
        let (chunk_tx, chunk_rx) = sync_channel(2);
        chunk_tx
            .send(NetworkChunk::Data(b"abcd\x01StreamTitle='X';efgh".to_vec()))
            .unwrap();
        chunk_tx.send(NetworkChunk::Eof).unwrap();
        let (event_tx, mut event_rx) = tokio_mpsc::unbounded_channel();
        let mut reader = IcyReader::new(
            chunk_rx,
            Some(4),
            event_tx,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let mut audio = [0_u8; 8];
        assert_eq!(reader.read_audio(&mut audio).unwrap(), 8);
        assert_eq!(&audio, b"abcdefgh");
        assert!(matches!(event_rx.try_recv(), Ok(AudioEvent::Metadata(title)) if title == "X"));
    }
}
