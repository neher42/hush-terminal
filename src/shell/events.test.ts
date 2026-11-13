import { describe, expect, it } from 'vitest';
import { parseMeta, parsePromptMarker, parseWorkingDirectory } from './events';

describe('shell events', () => {
  it('parses prompt markers', () => {
    expect(parsePromptMarker('A')).toEqual({ type: 'promptStart' });
    expect(parsePromptMarker('B')).toEqual({ type: 'commandInputStart' });
    expect(parsePromptMarker('D;127')).toEqual({ type: 'commandEnd', exitCode: 127 });
    expect(parsePromptMarker('D')).toEqual({ type: 'commandEnd', exitCode: null });
    expect(parsePromptMarker('Z')).toBeNull();
  });

  it('decodes working directory URLs', () => {
    expect(parseWorkingDirectory('file://mac.local/Users/me/My%20Stuff')).toEqual({
      type: 'cwd',
      host: 'mac.local',
      path: '/Users/me/My Stuff',
    });
    expect(parseWorkingDirectory('http://x')).toBeNull();
  });

  it('splits metadata at the first equals sign', () => {
    expect(parseMeta('path=/bin:/usr/bin')).toEqual({ type: 'meta', key: 'path', value: '/bin:/usr/bin' });
    expect(parseMeta('a=b=c')).toEqual({ type: 'meta', key: 'a', value: 'b=c' });
    expect(parseMeta('nothing')).toBeNull();
  });
});
