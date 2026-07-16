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
    importSkin: "Import local skin", browseSkins: "Browse skins", volume: "Volume", balance: "Balance", seek: "Seek",
    shuffle: "Shuffle", repeat: "Repeat", eq: "EQ", live: "LIVE", noTrack: "No track loaded",
    add: "Add", remove: "Remove", clear: "Clear", save: "Save", crop: "Crop",
    preamp: "Preamp", enabled: "On", disabled: "Off", unavailable: "Unavailable",
    streamUrlPrompt: "Enter an HTTP(S) audio stream URL:", error: "Error",
    emptyPlaylist: "Drop audio files here or use Add.", close: "Close", quit: "Quit Tonelag",
    alwaysOnTop: "Always on top", elapsed: "Elapsed time", remaining: "Remaining time", visualization: "Toggle visualization", language: "Switch to Norwegian",
    tonelagMenu: "Tonelag menu", doubleSize: "Double size", winshade: "Winshade",
    skinBrowser: "Skin browser", skinSearch: "Search classic skins", search: "Search",
    installedSkins: "Installed skins",
    useSkin: "Use", useSelectedSkin: "Use selected skin", deleteSkin: "Delete", deleteSelectedSkin: "Delete selected skin",
    deleteSkinConfirm: "Delete {{name}} permanently?", bundled: "bundled", bundledSkinCannotDelete: "Bundled skins are part of Tonelag and cannot be deleted.",
    clearSearch: "Clear search", loading: "Loading…", install: "Install", installing: "Installing…", installed: "Installed",
    previousPage: "Previous page", nextPage: "Next page", noSkins: "No skins found.",
    skinCatalogAttribution: "Catalog and previews from the Webamp Skin Museum.",
    skinLicenseNotice: "Skins are third-party works. Check the author's terms before redistributing them.",
    skinResults: "Showing {{start}}–{{end}}{{total}}", skinTotal: " of {{count}}",
  },
};

const nb = {
  translation: {
    appName: "Tonelag",
    main: "Hovedspiller", equalizer: "Equalizer", playlist: "Spilleliste",
    previous: "Forrige", play: "Spill", pause: "Pause", stop: "Stopp", next: "Neste",
    open: "Åpne", openFiles: "Åpne filer", openFolder: "Åpne mappe", openUrl: "Åpne URL",
    importSkin: "Importer lokalt skin", browseSkins: "Bla gjennom skins", volume: "Volum", balance: "Balanse", seek: "Spol",
    shuffle: "Tilfeldig", repeat: "Gjenta", eq: "EQ", live: "LIVE", noTrack: "Ingen lyd valgt",
    add: "Legg til", remove: "Fjern", clear: "Tøm", save: "Lagre", crop: "Beskjær",
    preamp: "Forforsterker", enabled: "På", disabled: "Av", unavailable: "Utilgjengelig",
    streamUrlPrompt: "Skriv inn en HTTP(S)-URL til en lydstrøm:", error: "Feil",
    emptyPlaylist: "Slipp lydfiler her eller bruk Legg til.", close: "Lukk", quit: "Avslutt Tonelag",
    alwaysOnTop: "Alltid øverst", elapsed: "Forløpt tid", remaining: "Gjenstående tid", visualization: "Bytt visualisering", language: "Bytt til engelsk",
    tonelagMenu: "Tonelag-meny", doubleSize: "Dobbel størrelse", winshade: "Rull opp",
    skinBrowser: "Skin-katalog", skinSearch: "Søk etter klassiske skins", search: "Søk",
    installedSkins: "Installerte skins",
    useSkin: "Bruk", useSelectedSkin: "Bruk valgt skin", deleteSkin: "Slett", deleteSelectedSkin: "Slett valgt skin",
    deleteSkinConfirm: "Vil du slette {{name}} permanent?", bundled: "bundlet", bundledSkinCannotDelete: "Bundlede skins er en del av Tonelag og kan ikke slettes.",
    clearSearch: "Nullstill søk", loading: "Laster …", install: "Installer", installing: "Installerer …", installed: "Installert",
    previousPage: "Forrige side", nextPage: "Neste side", noSkins: "Ingen skins funnet.",
    skinCatalogAttribution: "Katalog og forhåndsvisninger fra Webamp Skin Museum.",
    skinLicenseNotice: "Skins er laget av tredjeparter. Sjekk opphaverens vilkår før videre distribusjon.",
    skinResults: "Viser {{start}}–{{end}}{{total}}", skinTotal: " av {{count}}",
  },
};

void i18n.use(initReactI18next).init({ resources: { en, nb, no: nb }, lng: "en", fallbackLng: "en", interpolation: { escapeValue: false } });

export default i18n;
