export function useFormUtils() {
  function parseLines(value: string): string[] | undefined {
    const lines = value
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
    return lines.length > 0 ? lines : undefined;
  }

  function toText(list?: string[]) {
    return list?.join('\n') ?? '';
  }

  function normalizeOptional(value: string): string | undefined {
    const trimmed = value.trim();
    return trimmed ? trimmed : undefined;
  }

  function parseNumber(value: string): number | undefined {
    const trimmed = value.trim();
    if (!trimmed) {
      return undefined;
    }
    const num = Number(trimmed);
    return Number.isFinite(num) ? num : undefined;
  }

  return {
    parseLines,
    toText,
    normalizeOptional,
    parseNumber,
  };
}
