use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type};
use cpal::{
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ffmpeg::{channel_layout::ChannelLayout, codec, format, frame, media, software::resampling};
use ffmpeg_next as ffmpeg;
use rtrb::{Consumer, Producer, RingBuffer};
use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use tokio::sync::mpsc as tokio_mpsc;

use crate::model::{EQ_FREQUENCIES, EqSettings, QueueItem, QueueOrigin, SourceCapabilities};

const BUFFER_SECONDS: usize = 4;
const ANALYSIS_SIZE: usize = 2_048;

#[derive(Clone, Debug)]
pub enum AudioCommand {
    Load(QueueItem),
    Play,
    Pause,
    Stop,
    Seek(u64),
    SetVolume(f32),
    SetBalance(f32),
    SetEq(EqSettings),
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum AudioEvent {
    Loading,
    Buffering,
    Loaded {
        duration_ms: Option<u64>,
        bitrate_kbps: Option<u32>,
        sample_rate_hz: u32,
        channels: u16,
        title: Option<String>,
        artist: Option<String>,
        capabilities: SourceCapabilities,
    },
    Playing,
    Paused,
    Stopped,
    Position(u64),
    Spectrum(Vec<f32>),
    Metadata(String),
    Ended,
    Error(String),
}

struct SharedAudioState {
    paused: AtomicBool,
    active: AtomicBool,
    flush: AtomicBool,
    volume_bits: AtomicU32,
    balance_bits: AtomicU32,
    played_samples: AtomicU64,
    base_position_ms: AtomicU64,
    eq: Mutex<EqSettings>,
    cancel_network: Arc<AtomicBool>,
}

impl SharedAudioState {
    fn new(volume: f32, balance: f32, eq: EqSettings) -> Self {
        Self {
            paused: AtomicBool::new(true),
            active: AtomicBool::new(false),
            flush: AtomicBool::new(false),
            volume_bits: AtomicU32::new(volume.to_bits()),
            balance_bits: AtomicU32::new(balance.to_bits()),
            played_samples: AtomicU64::new(0),
            base_position_ms: AtomicU64::new(0),
            eq: Mutex::new(eq),
            cancel_network: Arc::new(AtomicBool::new(false)),
        }
    }

    fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    fn balance(&self) -> f32 {
        f32::from_bits(self.balance_bits.load(Ordering::Relaxed))
    }
}

pub struct AudioController {
    commands: Sender<AudioCommand>,
    shared: Arc<SharedAudioState>,
    sample_rate: u32,
    _stream: Stream,
}

impl AudioController {
    pub fn new(
        initial_volume: f32,
        initial_balance: f32,
        initial_eq: EqSettings,
    ) -> Result<(Self, tokio_mpsc::UnboundedReceiver<AudioEvent>)> {
        ffmpeg::init().context("failed initializing FFmpeg")?;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no default audio output device is available")?;
        let supported = device
            .default_output_config()
            .context("failed reading the default audio output configuration")?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;

        let capacity = sample_rate as usize * 2 * BUFFER_SECONDS;
        let (producer, consumer) = RingBuffer::<f32>::new(capacity);
        let shared = Arc::new(SharedAudioState::new(
            initial_volume,
            initial_balance,
            initial_eq,
        ));
        let (event_tx, event_rx) = tokio_mpsc::unbounded_channel();
        let stream = build_output_stream(
            &device,
            &config,
            sample_format,
            channels,
            consumer,
            shared.clone(),
            event_tx.clone(),
        )?;
        stream.play().context("failed starting the audio output")?;

        let (command_tx, command_rx) = mpsc::channel();
        let decoder_shared = shared.clone();
        thread::Builder::new()
            .name("tonelag-audio-decoder".into())
            .spawn(move || {
                decoder_loop(producer, command_rx, event_tx, decoder_shared, sample_rate)
            })?;

        Ok((
            Self {
                commands: command_tx,
                shared,
                sample_rate,
                _stream: stream,
            },
            event_rx,
        ))
    }

    pub fn send(&self, command: AudioCommand) -> Result<()> {
        match &command {
            AudioCommand::Play => self.shared.paused.store(false, Ordering::Release),
            AudioCommand::Pause => self.shared.paused.store(true, Ordering::Release),
            AudioCommand::Stop | AudioCommand::Load(_) => {
                self.shared.cancel_network.store(true, Ordering::Release);
                self.shared.flush.store(true, Ordering::Release);
                self.shared.played_samples.store(0, Ordering::Release);
                self.shared.base_position_ms.store(0, Ordering::Release);
            }
            AudioCommand::SetVolume(value) => self
                .shared
                .volume_bits
                .store(value.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed),
            AudioCommand::SetBalance(value) => self
                .shared
                .balance_bits
                .store(value.clamp(-1.0, 1.0).to_bits(), Ordering::Relaxed),
            AudioCommand::SetEq(eq) => {
                *self.shared.eq.lock().expect("eq lock poisoned") = eq.clone();
            }
            AudioCommand::Seek(_) | AudioCommand::Shutdown => {}
        }
        self.commands
            .send(command)
            .map_err(|_| anyhow!("audio decoder thread is unavailable"))
    }

    pub fn position_ms(&self) -> u64 {
        self.shared.base_position_ms.load(Ordering::Relaxed)
            + self.shared.played_samples.load(Ordering::Relaxed) * 1_000 / self.sample_rate as u64
    }
}

impl Drop for AudioController {
    fn drop(&mut self) {
        let _ = self.commands.send(AudioCommand::Shutdown);
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    channels: usize,
    consumer: Consumer<f32>,
    shared: Arc<SharedAudioState>,
    events: tokio_mpsc::UnboundedSender<AudioEvent>,
) -> Result<Stream> {
    let stream = match format {
        SampleFormat::F32 => {
            build_typed_output_stream::<f32>(device, config, channels, consumer, shared, events)?
        }
        SampleFormat::I16 => {
            build_typed_output_stream::<i16>(device, config, channels, consumer, shared, events)?
        }
        SampleFormat::U16 => {
            build_typed_output_stream::<u16>(device, config, channels, consumer, shared, events)?
        }
        other => return Err(anyhow!("unsupported output sample format: {other}")),
    };
    Ok(stream)
}

fn build_typed_output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    mut consumer: Consumer<f32>,
    shared: Arc<SharedAudioState>,
    events: tokio_mpsc::UnboundedSender<AudioEvent>,
) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let error_callback = move |error| {
        let _ = events.send(AudioEvent::Error(format!("audio output error: {error}")));
    };
    Ok(device.build_output_stream(
        *config,
        move |data: &mut [T], _| fill_output(data, channels, &mut consumer, &shared),
        error_callback,
        None,
    )?)
}

fn fill_output<T>(
    data: &mut [T],
    channels: usize,
    consumer: &mut Consumer<f32>,
    shared: &SharedAudioState,
) where
    T: SizedSample + FromSample<f32>,
{
    if shared.flush.swap(false, Ordering::AcqRel) {
        while consumer.pop().is_ok() {}
    }
    let paused = shared.paused.load(Ordering::Acquire);
    let active = shared.active.load(Ordering::Acquire);
    let volume = shared.volume();
    let balance = shared.balance();
    let left_gain = volume * (1.0 - balance.max(0.0));
    let right_gain = volume * (1.0 + balance.min(0.0));
    let mut consumed_frames = 0_u64;

    for frame in data.chunks_mut(channels.max(1)) {
        let (left, right) = if paused || !active {
            (0.0, 0.0)
        } else {
            match (consumer.pop(), consumer.pop()) {
                (Ok(left), Ok(right)) => {
                    consumed_frames += 1;
                    (left * left_gain, right * right_gain)
                }
                _ => (0.0, 0.0),
            }
        };
        if frame.len() == 1 {
            frame[0] = T::from_sample(((left + right) * 0.5).clamp(-1.0, 1.0));
        } else {
            frame[0] = T::from_sample(left.clamp(-1.0, 1.0));
            frame[1] = T::from_sample(right.clamp(-1.0, 1.0));
            for sample in &mut frame[2..] {
                *sample = T::from_sample(0.0);
            }
        }
    }
    shared
        .played_samples
        .fetch_add(consumed_frames, Ordering::Relaxed);
}

enum DecodeAction {
    Idle,
    Load(QueueItem),
    Shutdown,
}

fn decoder_loop(
    mut producer: Producer<f32>,
    commands: Receiver<AudioCommand>,
    events: tokio_mpsc::UnboundedSender<AudioEvent>,
    shared: Arc<SharedAudioState>,
    output_rate: u32,
) {
    let mut next = DecodeAction::Idle;
    loop {
        let action = match next {
            DecodeAction::Idle => match commands.recv() {
                Ok(AudioCommand::Load(item)) => DecodeAction::Load(item),
                Ok(AudioCommand::Shutdown) | Err(_) => DecodeAction::Shutdown,
                Ok(command) => {
                    handle_idle_command(command, &events, &shared);
                    DecodeAction::Idle
                }
            },
            other => other,
        };

        next = match action {
            DecodeAction::Load(item) => {
                let _ = events.send(AudioEvent::Loading);
                let stream = matches!(item.origin, QueueOrigin::HttpStream { .. });
                let mut retry = 0_usize;
                loop {
                    match decode_item(
                        &item,
                        &mut producer,
                        &commands,
                        &events,
                        &shared,
                        output_rate,
                    ) {
                        Ok(action) => break action,
                        Err(_error) if stream && retry < 3 => {
                            shared.active.store(false, Ordering::Release);
                            let _ = events.send(AudioEvent::Buffering);
                            let delay = [1, 2, 4][retry];
                            retry += 1;
                            match wait_for_reconnect(&commands, &events, &shared, delay) {
                                Some(action) => break action,
                                None => continue,
                            }
                        }
                        Err(error) => {
                            shared.active.store(false, Ordering::Release);
                            let _ = events.send(AudioEvent::Error(error.to_string()));
                            break DecodeAction::Idle;
                        }
                    }
                }
            }
            DecodeAction::Shutdown => break,
            DecodeAction::Idle => DecodeAction::Idle,
        };
    }
}

fn wait_for_reconnect(
    commands: &Receiver<AudioCommand>,
    events: &tokio_mpsc::UnboundedSender<AudioEvent>,
    shared: &SharedAudioState,
    seconds: u64,
) -> Option<DecodeAction> {
    let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match commands.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(AudioCommand::Load(item)) => return Some(DecodeAction::Load(item)),
            Ok(AudioCommand::Stop) => {
                shared.paused.store(true, Ordering::Release);
                let _ = events.send(AudioEvent::Stopped);
                return Some(DecodeAction::Idle);
            }
            Ok(AudioCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Some(DecodeAction::Shutdown);
            }
            Ok(command) => handle_idle_command(command, events, shared),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn handle_idle_command(
    command: AudioCommand,
    events: &tokio_mpsc::UnboundedSender<AudioEvent>,
    shared: &SharedAudioState,
) {
    match command {
        AudioCommand::Play => {
            shared.paused.store(false, Ordering::Release);
            let _ = events.send(AudioEvent::Playing);
        }
        AudioCommand::Pause => {
            shared.paused.store(true, Ordering::Release);
            let _ = events.send(AudioEvent::Paused);
        }
        AudioCommand::Stop => {
            shared.active.store(false, Ordering::Release);
            let _ = events.send(AudioEvent::Stopped);
        }
        _ => {}
    }
}

fn decode_item(
    item: &QueueItem,
    producer: &mut Producer<f32>,
    commands: &Receiver<AudioCommand>,
    events: &tokio_mpsc::UnboundedSender<AudioEvent>,
    shared: &Arc<SharedAudioState>,
    output_rate: u32,
) -> Result<DecodeAction> {
    let is_live = matches!(item.origin, QueueOrigin::HttpStream { .. });
    shared.cancel_network.store(false, Ordering::Release);
    let mut input_owner = match &item.origin {
        QueueOrigin::LocalFile { path } => crate::http_io::open_file(path)?,
        QueueOrigin::HttpStream { url } => {
            crate::http_io::open_http(url, events.clone(), shared.cancel_network.clone())?
        }
    };
    let ictx = input_owner.context_mut();
    let input_stream = ictx
        .streams()
        .best(media::Type::Audio)
        .context("input does not contain an audio stream")?;
    let stream_index = input_stream.index();
    let time_base = input_stream.time_base();
    let codec_context = codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = codec_context.decoder().audio()?;
    let bitrate_kbps = u32::try_from(ictx.bit_rate())
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value / 1_000);
    let sample_rate_hz = decoder.rate();
    let channels = decoder.channels();
    let duration_ms = if is_live || ictx.duration() <= 0 {
        None
    } else {
        Some((ictx.duration() as u64) / 1_000)
    };
    let title = ictx
        .metadata()
        .get("title")
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let artist = ictx
        .metadata()
        .get("artist")
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let input_layout = if decoder.channel_layout().is_empty() {
        ChannelLayout::default(decoder.channels() as i32)
    } else {
        decoder.channel_layout()
    };
    let mut resampler = resampling::Context::get(
        decoder.format(),
        input_layout,
        decoder.rate(),
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
        ChannelLayout::STEREO,
        output_rate,
    )?;
    let capabilities = SourceCapabilities {
        seekable: !is_live,
        live: is_live,
        dsp_allowed: true,
        analysis_allowed: true,
        supports_metadata_updates: is_live,
    };
    shared.active.store(false, Ordering::Release);
    shared.paused.store(false, Ordering::Release);
    let _ = events.send(AudioEvent::Loaded {
        duration_ms,
        bitrate_kbps,
        sample_rate_hz,
        channels,
        title,
        artist,
        capabilities,
    });
    if is_live {
        let _ = events.send(AudioEvent::Buffering);
    }

    let desired_buffer_ms = if is_live {
        match bitrate_kbps {
            Some(rate) if rate <= 64 => 1_500,
            Some(rate) if rate >= 192 => 750,
            _ => 1_000,
        }
    } else {
        150
    };
    let buffer_ms = duration_ms
        .map(|duration| desired_buffer_ms.min((duration / 2).max(1)))
        .unwrap_or(desired_buffer_ms);
    let prebuffer_samples = output_rate as usize * 2 * buffer_ms as usize / 1_000;
    let ring_capacity = output_rate as usize * 2 * BUFFER_SECONDS;
    let mut buffered_samples = 0_usize;

    let mut eq = EqProcessor::new(
        output_rate,
        shared.eq.lock().expect("eq lock poisoned").clone(),
    );
    let mut analyzer = SpectrumAnalyzer::new(output_rate);
    let mut decoded = frame::Audio::empty();
    let mut metadata_title = String::new();

    loop {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(ictx) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => break,
            Err(error) => return Err(error.into()),
        }
        if packet.stream() != stream_index {
            continue;
        }
        if let Some(action) =
            drain_commands(commands, events, shared, ictx, &mut decoder, time_base)?
        {
            return Ok(action);
        }
        while shared.paused.load(Ordering::Acquire) {
            match commands.recv_timeout(Duration::from_millis(50)) {
                Ok(command) => {
                    if let Some(action) =
                        apply_command(command, events, shared, ictx, &mut decoder, time_base)?
                    {
                        return Ok(action);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(DecodeAction::Shutdown),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }

        decoder.send_packet(&packet)?;
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut resampled = frame::Audio::empty();
            resampler.run(&decoded, &mut resampled)?;
            let bytes = resampled.data(0);
            let mut samples = bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect::<Vec<_>>();
            let settings = shared.eq.lock().expect("eq lock poisoned").clone();
            eq.update_if_needed(settings);
            eq.process(&mut samples);
            if let Some(spectrum) = analyzer.push(&samples) {
                let _ = events.send(AudioEvent::Spectrum(spectrum));
            }
            let frame_samples = samples.len();
            for sample in samples {
                let mut value = sample;
                loop {
                    match producer.push(value) {
                        Ok(()) => break,
                        Err(rtrb::PushError::Full(returned)) => {
                            value = returned;
                            if let Some(action) = drain_commands(
                                commands,
                                events,
                                shared,
                                ictx,
                                &mut decoder,
                                time_base,
                            )? {
                                return Ok(action);
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                    }
                }
            }
            buffered_samples += frame_samples;
            if !shared.active.load(Ordering::Acquire) && buffered_samples >= prebuffer_samples {
                shared.active.store(true, Ordering::Release);
                if !shared.paused.load(Ordering::Acquire) {
                    let _ = events.send(AudioEvent::Playing);
                }
            }
        }

        if is_live {
            let metadata = ictx.metadata();
            let title = metadata
                .get("StreamTitle")
                .or_else(|| metadata.get("title"))
                .unwrap_or_default()
                .to_owned();
            if !title.is_empty() && title != metadata_title {
                metadata_title = title;
                let _ = events.send(AudioEvent::Metadata(metadata_title.clone()));
            }
        }
    }

    decoder.send_eof()?;
    if is_live {
        shared.active.store(false, Ordering::Release);
        Err(anyhow!("stream ended unexpectedly"))
    } else {
        if !shared.active.swap(true, Ordering::AcqRel) && !shared.paused.load(Ordering::Acquire) {
            let _ = events.send(AudioEvent::Playing);
        }
        while producer.slots() < ring_capacity {
            if let Some(action) =
                drain_commands(commands, events, shared, ictx, &mut decoder, time_base)?
            {
                return Ok(action);
            }
            thread::sleep(Duration::from_millis(2));
        }
        shared.active.store(false, Ordering::Release);
        let _ = events.send(AudioEvent::Ended);
        Ok(DecodeAction::Idle)
    }
}

fn drain_commands(
    commands: &Receiver<AudioCommand>,
    events: &tokio_mpsc::UnboundedSender<AudioEvent>,
    shared: &SharedAudioState,
    input: &mut format::context::Input,
    decoder: &mut codec::decoder::Audio,
    time_base: ffmpeg::Rational,
) -> Result<Option<DecodeAction>> {
    loop {
        match commands.try_recv() {
            Ok(command) => {
                if let Some(action) =
                    apply_command(command, events, shared, input, decoder, time_base)?
                {
                    return Ok(Some(action));
                }
            }
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => return Ok(Some(DecodeAction::Shutdown)),
        }
    }
}

fn apply_command(
    command: AudioCommand,
    events: &tokio_mpsc::UnboundedSender<AudioEvent>,
    shared: &SharedAudioState,
    input: &mut format::context::Input,
    decoder: &mut codec::decoder::Audio,
    time_base: ffmpeg::Rational,
) -> Result<Option<DecodeAction>> {
    match command {
        AudioCommand::Load(item) => Ok(Some(DecodeAction::Load(item))),
        AudioCommand::Shutdown => Ok(Some(DecodeAction::Shutdown)),
        AudioCommand::Stop => {
            shared.paused.store(true, Ordering::Release);
            shared.active.store(false, Ordering::Release);
            shared.flush.store(true, Ordering::Release);
            let _ = events.send(AudioEvent::Stopped);
            Ok(Some(DecodeAction::Idle))
        }
        AudioCommand::Play => {
            shared.paused.store(false, Ordering::Release);
            if shared.active.load(Ordering::Acquire) {
                let _ = events.send(AudioEvent::Playing);
            }
            Ok(None)
        }
        AudioCommand::Pause => {
            shared.paused.store(true, Ordering::Release);
            let _ = events.send(AudioEvent::Paused);
            Ok(None)
        }
        AudioCommand::Seek(position_ms) => {
            let timestamp = (position_ms as i64 * time_base.denominator() as i64)
                / (1_000 * time_base.numerator() as i64);
            input.seek(timestamp, ..timestamp)?;
            decoder.flush();
            shared.flush.store(true, Ordering::Release);
            shared.played_samples.store(0, Ordering::Release);
            shared
                .base_position_ms
                .store(position_ms, Ordering::Release);
            Ok(None)
        }
        AudioCommand::SetVolume(value) => {
            shared
                .volume_bits
                .store(value.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
            Ok(None)
        }
        AudioCommand::SetBalance(value) => {
            shared
                .balance_bits
                .store(value.clamp(-1.0, 1.0).to_bits(), Ordering::Relaxed);
            Ok(None)
        }
        AudioCommand::SetEq(eq) => {
            *shared.eq.lock().expect("eq lock poisoned") = eq;
            Ok(None)
        }
    }
}

struct EqProcessor {
    sample_rate: u32,
    settings: EqSettings,
    left: Vec<DirectForm1<f32>>,
    right: Vec<DirectForm1<f32>>,
}

impl EqProcessor {
    fn new(sample_rate: u32, settings: EqSettings) -> Self {
        let mut processor = Self {
            sample_rate,
            settings: EqSettings::default(),
            left: Vec::new(),
            right: Vec::new(),
        };
        processor.rebuild(settings);
        processor
    }

    fn update_if_needed(&mut self, settings: EqSettings) {
        if self.settings != settings {
            self.rebuild(settings);
        }
    }

    fn rebuild(&mut self, settings: EqSettings) {
        self.left.clear();
        self.right.clear();
        for (frequency, gain) in EQ_FREQUENCIES.iter().zip(settings.bands_db) {
            if let Ok(coefficients) = Coefficients::<f32>::from_params(
                Type::PeakingEQ(gain),
                (self.sample_rate as f32).hz(),
                (*frequency as f32).hz(),
                1.4,
            ) {
                self.left.push(DirectForm1::new(coefficients));
                self.right.push(DirectForm1::new(coefficients));
            }
        }
        self.settings = settings;
    }

    fn process(&mut self, samples: &mut [f32]) {
        if !self.settings.enabled {
            return;
        }
        let preamp = 10_f32.powf(self.settings.preamp_db / 20.0);
        for frame in samples.chunks_exact_mut(2) {
            let mut left = frame[0] * preamp;
            let mut right = frame[1] * preamp;
            for filter in &mut self.left {
                left = filter.run(left);
            }
            for filter in &mut self.right {
                right = filter.run(right);
            }
            frame[0] = left;
            frame[1] = right;
        }
    }
}

struct SpectrumAnalyzer {
    sample_rate: u32,
    samples: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
    last_emit: std::time::Instant,
}

impl SpectrumAnalyzer {
    fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::new();
        Self {
            sample_rate,
            samples: Vec::with_capacity(ANALYSIS_SIZE),
            fft: planner.plan_fft_forward(ANALYSIS_SIZE),
            last_emit: std::time::Instant::now() - Duration::from_millis(34),
        }
    }

    fn push(&mut self, interleaved: &[f32]) -> Option<Vec<f32>> {
        for frame in interleaved.chunks_exact(2) {
            self.samples.push((frame[0] + frame[1]) * 0.5);
            if self.samples.len() == ANALYSIS_SIZE {
                let should_emit = self.last_emit.elapsed() >= Duration::from_micros(33_334);
                let result = should_emit.then(|| self.analyze());
                self.samples.clear();
                if result.is_some() {
                    self.last_emit = std::time::Instant::now();
                }
                return result;
            }
        }
        None
    }

    fn analyze(&self) -> Vec<f32> {
        let mut buffer = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let window =
                    0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / ANALYSIS_SIZE as f32).cos();
                Complex32::new(sample * window, 0.0)
            })
            .collect::<Vec<_>>();
        self.fft.process(&mut buffer);

        let mut bars = Vec::with_capacity(19);
        for bar in 0..19 {
            let low_hz = 45.0 * (16_000.0_f32 / 45.0).powf(bar as f32 / 19.0);
            let high_hz = 45.0 * (16_000.0_f32 / 45.0).powf((bar + 1) as f32 / 19.0);
            let low = ((low_hz * ANALYSIS_SIZE as f32 / self.sample_rate as f32) as usize)
                .clamp(1, ANALYSIS_SIZE / 2 - 1);
            let high = ((high_hz * ANALYSIS_SIZE as f32 / self.sample_rate as f32) as usize)
                .clamp(low + 1, ANALYSIS_SIZE / 2);
            let magnitude = buffer[low..high]
                .iter()
                .map(|value| value.norm())
                .fold(0.0_f32, f32::max)
                / ANALYSIS_SIZE as f32;
            bars.push((magnitude * 12.0).clamp(0.0, 1.0));
        }
        bars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equalizer_is_bypass_when_disabled() {
        let mut processor = EqProcessor::new(48_000, EqSettings::default());
        let mut samples = vec![0.25, -0.25, 0.5, -0.5];
        let original = samples.clone();
        processor.process(&mut samples);
        assert_eq!(samples, original);
    }

    #[test]
    fn analyzer_produces_nineteen_finite_bars() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let mut input = Vec::new();
        for index in 0..ANALYSIS_SIZE {
            let sample = (std::f32::consts::TAU * 440.0 * index as f32 / 48_000.0).sin();
            input.extend([sample, sample]);
        }
        let bars = analyzer.push(&input).unwrap();
        assert_eq!(bars.len(), 19);
        assert!(bars.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn equalizer_bands_are_within_one_decibel() {
        let sample_rate = 48_000;
        for (band_index, frequency) in EQ_FREQUENCIES.iter().enumerate() {
            let mut settings = EqSettings {
                enabled: true,
                ..EqSettings::default()
            };
            settings.bands_db[band_index] = 6.0;
            let mut processor = EqProcessor::new(sample_rate, settings);
            let frames = sample_rate as usize * 2;
            let mut samples = Vec::with_capacity(frames * 2);
            for index in 0..frames {
                let value = 0.05
                    * (std::f32::consts::TAU * *frequency as f32 * index as f32
                        / sample_rate as f32)
                        .sin();
                samples.extend([value, value]);
            }
            processor.process(&mut samples);
            let settled = &samples[sample_rate as usize * 2..];
            let output_rms = (settled
                .iter()
                .step_by(2)
                .map(|value| value * value)
                .sum::<f32>()
                / (settled.len() / 2) as f32)
                .sqrt();
            let input_rms = 0.05 / 2.0_f32.sqrt();
            let measured_db = 20.0 * (output_rms / input_rms).log10();
            assert!(
                (measured_db - 6.0).abs() <= 1.0,
                "band {frequency} Hz measured {measured_db:.2} dB"
            );
            assert!(settled.iter().all(|sample| sample.abs() < 1.0));
        }
    }

    #[test]
    #[ignore = "requires a physical audio output device"]
    fn hardware_audio_smoke_test() {
        let path = std::env::var("TONELAG_SMOKE_FILE")
            .expect("set TONELAG_SMOKE_FILE to a short audio fixture");
        let item = QueueItem {
            id: uuid::Uuid::new_v4(),
            origin: QueueOrigin::LocalFile { path },
            title: "Hardware smoke tone".into(),
            artist: None,
            duration_ms: None,
            available: true,
        };
        let (controller, mut events) =
            AudioController::new(0.05, 0.0, EqSettings::default()).unwrap();
        controller.send(AudioCommand::Load(item)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut played = false;
        while std::time::Instant::now() < deadline {
            while let Ok(event) = events.try_recv() {
                if matches!(event, AudioEvent::Playing) {
                    played = true;
                }
            }
            if played && controller.position_ms() >= 250 {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let consumed_position = controller.position_ms();
        controller.send(AudioCommand::Stop).unwrap();
        assert!(played, "decoder never entered Playing state");
        assert!(
            consumed_position >= 250,
            "audio callback did not consume frames"
        );
    }
}
