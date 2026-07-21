import { describe, expect, it } from 'vitest';
import { formatKilobytes } from './format';

describe('formatKilobytes', () => {
  it('formats bundle sizes consistently', () => {
    expect(formatKilobytes(0)).toBe('0.0 KB');
    expect(formatKilobytes(1536)).toBe('1.5 KB');
  });
});
