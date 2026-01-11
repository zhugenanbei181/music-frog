import { describe, expect, it } from 'vitest';
import { useFormUtils } from '../useFormUtils';

describe('useFormUtils', () => {
  it('parses non-empty lines', () => {
    const { parseLines } = useFormUtils();
    expect(parseLines('a\n\n b \n')).toEqual(['a', 'b']);
    expect(parseLines('   ')).toBeUndefined();
  });

  it('serializes lists to text', () => {
    const { toText } = useFormUtils();
    expect(toText(['a', 'b'])).toBe('a\nb');
    expect(toText(undefined)).toBe('');
  });

  it('normalizes optional strings', () => {
    const { normalizeOptional } = useFormUtils();
    expect(normalizeOptional('  ok ')).toBe('ok');
    expect(normalizeOptional('   ')).toBeUndefined();
  });

  it('parses numeric values safely', () => {
    const { parseNumber } = useFormUtils();
    expect(parseNumber('42')).toBe(42);
    expect(parseNumber('  ')).toBeUndefined();
    expect(parseNumber('NaN')).toBeUndefined();
  });
});
