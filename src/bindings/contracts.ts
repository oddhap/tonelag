// Rust owns these declarations. Run `pnpm bindings` after changing the Rust models.
export type { AppSnapshot } from "./generated/AppSnapshot";
export type { EqSettings } from "./generated/EqSettings";
export type { PlaybackSnapshot } from "./generated/PlaybackSnapshot";
export type { PlaybackStatus } from "./generated/PlaybackStatus";
export type { PlayerCommand } from "./generated/PlayerCommand";
export type { QueueItem } from "./generated/QueueItem";
export type { QueueOrigin } from "./generated/QueueOrigin";
export type { ResolvedStream } from "./generated/ResolvedStream";
export type { Settings } from "./generated/Settings";
export type { SkinDescriptor } from "./generated/SkinDescriptor";
export type { SkinCatalogEntry } from "./generated/SkinCatalogEntry";
export type { SkinCatalogPage } from "./generated/SkinCatalogPage";
export type { SourceCapabilities } from "./generated/SourceCapabilities";
export type { WindowLayout } from "./generated/WindowLayout";
export type { WindowPoint } from "./generated/WindowPoint";

export type Uuid = string;
