import type { PlannedOperation, ReportEntry } from '../api/commands';

/** Report entries grouped by key, in order of first appearance. */
export function groupBy(entries: ReportEntry[], key: (e: ReportEntry) => string): [string, ReportEntry[]][] {
    const groups = new Map<string, ReportEntry[]>();
    for (const e of entries) {
        const k = key(e);
        const list = groups.get(k);
        if (list) list.push(e);
        else groups.set(k, [e]);
    }
    return [...groups.entries()];
}

/**
 * A duplicate report entry is the kept copy when its rule trashes other files
 * in this plan and the entry itself is not trashed.
 */
export function isKeptCopy(entry: ReportEntry, results: PlannedOperation[]): boolean {
    return (
        entry.group !== null &&
        results.some((op) => op.operation === 'trash' && op.rule_name === entry.rule_name) &&
        !results.some((op) => op.source_path === entry.path)
    );
}
