interface SpectrumProps {
  values: number[];
  colors?: string[];
  mode?: "spectrum" | "oscilloscope";
}

export function Spectrum({ values, colors = [], mode = "spectrum" }: SpectrumProps) {
  return (
    <div className={`spectrum ${mode}`} aria-hidden="true">
      {values.slice(0, 19).map((value, index) => (
        <i
          key={index}
          style={{
            height: mode === "spectrum"
              ? `${Math.max(1, Math.min(100, value * 100))}%`
              : `${Math.max(1, Math.min(15, value * 15))}px`,
            backgroundColor: colors[index % Math.max(1, colors.length)] || undefined,
          }}
        />
      ))}
    </div>
  );
}
