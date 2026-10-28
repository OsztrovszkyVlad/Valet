import { describe, expect, it } from 'vitest';
import { splitPath } from './paths';

describe('splitPath', () => {
    it('keeps the last folder and the name in the tail', () => {
        expect(splitPath('C:\\Users\\ana\\Downloads\\a.pdf')).toEqual({
            head: 'C:\\Users\\ana\\',
            tail: 'Downloads\\a.pdf',
        });
        expect(splitPath('/home/ana/Downloads/Imagini')).toEqual({ head: '/home/ana/', tail: 'Downloads/Imagini' });
    });

    it('returns the whole path as tail when it is short', () => {
        expect(splitPath('a.pdf')).toEqual({ head: '', tail: 'a.pdf' });
        expect(splitPath('C:\\a.pdf')).toEqual({ head: '', tail: 'C:\\a.pdf' });
    });

    it('ignores a trailing separator and keeps head + tail equal to the path', () => {
        const p = 'C:\\Users\\ana\\Documents\\';
        const { head, tail } = splitPath(p);
        expect(head).toBe('C:\\Users\\');
        expect(head + tail).toBe(p);
    });
});
