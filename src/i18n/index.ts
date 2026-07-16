import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const en = {
  translation: {
    appName: "Tonelag",
    main: "Main player",
    equalizer: "Equalizer",
    playlist: "Playlist",
    previous: "Previous", play: "Play", pause: "Pause", stop: "Stop", next: "Next",
    open: "Open", openFiles: "Open files", openFolder: "Open folder", openUrl: "Open URL",
    importSkin: "Import skin", volume: "Volume", balance: "Balance", seek: "Seek",
    shuffle: "Shuffle", repeat: "Repeat", eq: "EQ", live: "LIVE", noTrack: "No track loaded",
    add: "Add", remove: "Remove", clear: "Clear", save: "Save", crop: "Crop",
    preamp: "Preamp", enabled: "On", disabled: "Off", unavailable: "Unavailable",
    streamUrlPrompt: "Enter an HTTP(S) audio stream URL:", error: "Error",
    emptyPlaylist: "Drop audio files here or use Add.", close: "Close",
    alwaysOnTop: "Always on top", elapsed: "Elapsed time", remaining: "Remaining time", visualization: "Toggle visualization", language: "Switch to Norwegian",
  },
};

const nb = {
  translation: {
    appName: "Tonelag",
    main: "Hovedspiller", equalizer: "Equalizer", playlist: "Spilleliste",
    previous: "Forrige", play: "Spill", pause: "Pause", stop: "Stopp", next: "Neste",
    open: "Åpne", openFiles: "Åpne filer", openFolder: "Åpne mappe", openUrl: "Åpne URL",
    importSkin: "Importer skin", volume: "Volum", balance: "Balanse", seek: "Spol",
    shuffle: "Tilfeldig", repeat: "Gjenta", eq: "EQ", live: "LIVE", noTrack: "Ingen lyd valgt",
    add: "Legg til", remove: "Fjern", clear: "Tøm", save: "Lagre", crop: "Beskjær",
    preamp: "Forforsterker", enabled: "På", disabled: "Av", unavailable: "Utilgjengelig",
    streamUrlPrompt: "Skriv inn en HTTP(S)-URL til en lydstrøm:", error: "Feil",
    emptyPlaylist: "Slipp lydfiler her eller bruk Legg til.", close: "Lukk",
    alwaysOnTop: "Alltid øverst", elapsed: "Forløpt tid", remaining: "Gjenstående tid", visualization: "Bytt visualisering", language: "Bytt til engelsk",
  },
};

void i18n.use(initReactI18next).init({ resources: { en, nb, no: nb }, lng: "en", fallbackLng: "en", interpolation: { escapeValue: false } });

export default i18n;
