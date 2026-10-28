import React from 'react';
import { useTranslation } from 'react-i18next';
import type { PlannedOperation, ReportEntry } from '../api/commands';
import { formatBytes } from '../i18n/format';
import { groupBy, isKeptCopy } from './reports';

interface ReportsSectionProps {
    reports: ReportEntry[];
    /** The planned operations of the same preview, to mark the copy a duplicate + trash rule keeps. */
    results: PlannedOperation[];
}

/** "Rapoarte": files report rules matched (large files, empty folders, ...) and duplicate groups. Nothing here is an operation. */
const ReportsSection: React.FC<ReportsSectionProps> = ({ reports, results }) => {
    const { t, i18n } = useTranslation();
    if (reports.length === 0) return null;

    const byRule = groupBy(
        reports.filter((r) => r.group === null),
        (r) => r.rule_name,
    );
    const duplicates = groupBy(
        reports.filter((r) => r.group !== null),
        (r) => r.group ?? '',
    );
    const anyKept = duplicates.some(([, entries]) => entries.some((e) => isKeptCopy(e, results)));

    return (
        <section className="reports" aria-labelledby="reports-title">
            <h3 id="reports-title" className="results-title">
                {t('preview.reportsTitle')}
            </h3>
            <p className="view-description">{t('preview.reportsHelp')}</p>

            {byRule.map(([ruleName, entries]) => (
                <div key={ruleName} className="report-group">
                    <h4>{ruleName}</h4>
                    <div className="results-table-container">
                        <table className="results-table">
                            <thead>
                                <tr>
                                    <th>{t('preview.colPath')}</th>
                                    <th>{t('preview.colSize')}</th>
                                </tr>
                            </thead>
                            <tbody>
                                {entries.map((e, i) => (
                                    <tr key={`${e.path}-${i}`}>
                                        <td>
                                            <code className="path-code">{e.path}</code>
                                        </td>
                                        <td className="report-size">{formatBytes(e.size, i18n.language)}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </div>
            ))}

            {duplicates.length > 0 && (
                <div className="report-group">
                    <h4>{t('preview.duplicatesTitle')}</h4>
                    {duplicates.map(([group, entries], n) => (
                        <div key={group} className="duplicate-group">
                            <h5>
                                {t('preview.duplicateGroup', {
                                    n: n + 1,
                                    size: formatBytes(entries[0].size, i18n.language),
                                })}
                            </h5>
                            <ul className="duplicate-list">
                                {entries.map((e, i) => (
                                    <li key={`${e.path}-${i}`}>
                                        <code className="path-code">{e.path}</code>
                                        {isKeptCopy(e, results) && <span className="badge">{t('preview.kept')}</span>}
                                    </li>
                                ))}
                            </ul>
                        </div>
                    ))}
                    {anyKept && <p className="form-help">{t('preview.keptNote')}</p>}
                </div>
            )}
        </section>
    );
};

export default ReportsSection;
