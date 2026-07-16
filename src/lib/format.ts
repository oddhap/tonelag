export function formatTime(milliseconds: number | null): string {
  if (milliseconds === null || !Number.isFinite(milliseconds)) return "--:--";
  const seconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

export const displayTitle = (artist: string | null, title: string) =>
  artist ? `${artist} – ${title}` : title;
