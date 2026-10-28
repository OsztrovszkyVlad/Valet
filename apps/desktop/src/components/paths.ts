/**
 * Splits a path into a head that may be shortened and a tail that stays
 * visible: the last `keep` segments. `head + tail === path`.
 */
export function splitPath(path: string, keep = 2): { head: string; tail: string } {
    // Index just after each separator, ignoring a trailing one.
    const trimmed = path.replace(/[\\/]+$/, '');
    const cuts: number[] = [];
    for (let i = 0; i < trimmed.length; i++) {
        if (trimmed[i] === '\\' || trimmed[i] === '/') cuts.push(i + 1);
    }
    if (cuts.length < keep) return { head: '', tail: path };
    const at = cuts[cuts.length - keep];
    return { head: path.slice(0, at), tail: path.slice(at) };
}
