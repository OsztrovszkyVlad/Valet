// Typed client for the Tauri commands. New UI code calls these, not `invoke`.
// Field names match the serde output of the Rust side.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** `valet_core::planner::SkipReason`, serialized camelCase. */
export type SkipReason =
    | 'excluded'
    | 'hidden'
    | 'reparsePoint'
    | 'beingWritten'
    | 'cloudPlaceholder'
    | 'conflict'
    | 'alreadyInPlace'
    | 'noExtension'
    | 'invalidDestination'
    | 'unsupportedAction'
    | 'unreadable'
    | 'ruleDestination';

export interface PlannedOperation {
    source_path: string;
    destination_path: string;
    rule_name: string;
    /** `trash` has an empty `destination_path`. */
    operation: 'move' | 'copy' | 'trash';
}

/** A file (or empty folder) a `report` rule matched, or the copy a duplicate + trash rule keeps. */
export interface ReportEntry {
    path: string;
    rule_id: string;
    rule_name: string;
    /** Bytes. */
    size: number;
    /** Duplicate group id (short content hash); null outside duplicate rules. */
    group: string | null;
}

export interface SkippedFile {
    path: string;
    reason: SkipReason;
    rule_name: string | null;
    /** For `noExtension`: the extension(s) the rule expects. Optional until the backend sends it. */
    ext?: string | null;
}

export interface DryRunResponse {
    results: PlannedOperation[];
    total_files: number;
    total_rules_applied: number;
    skipped: SkippedFile[];
    /** Not operations: nothing happens to these files. */
    reports: ReportEntry[];
    /** Id of the stored preview; `executeOperations` needs it. Single use. */
    plan_id: string;
}

/** Error string from `execute_operations` when the plan id has no stored preview. */
export const PREVIEW_REQUIRED = 'preview_required';

export interface ExecuteError {
    source_path: string;
    destination_path: string;
    rule_name: string;
    error_message: string;
}

export interface ExecuteResponse {
    executed_count: number;
    failed_operations: ExecuteError[];
    success: boolean;
    batch_id: string;
}

export type BatchKind = 'apply' | 'undo' | 'recovery';
export type OpState = 'planned' | 'in_progress' | 'done' | 'failed' | 'rolled_back';
/** `restore`: undo of a `trash` operation (back from the Recycle Bin to `source_path`). */
export type OpType = 'move' | 'copy' | 'trash' | 'restore';

export interface StateCounts {
    planned: number;
    in_progress: number;
    done: number;
    failed: number;
    rolled_back: number;
}

export interface BatchInfo {
    id: string;
    kind: BatchKind;
    /** UTC, ISO 8601. */
    created_at: string;
    finished_at: string | null;
    summary: string | null;
    counts: StateCounts;
}

export interface OpRecord {
    id: number;
    batch_id: string;
    seq: number;
    op_type: OpType;
    source_path: string;
    dest_path: string | null;
    size: number | null;
    hash_before: string | null;
    hash_after: string | null;
    state: OpState;
    /** `Code` or `Code: detail`, Code being a `FailCode`. */
    error: string | null;
    created_at: string;
    finished_at: string | null;
    undo_of: number | null;
}

export interface BatchDetail {
    batch: BatchInfo;
    operations: OpRecord[];
}

/** `valet_core::executor::FailCode`, serialized as the variant name. */
export type FailCode =
    | 'SourceMissing'
    | 'ChangedSincePreview'
    | 'CloudPlaceholder'
    | 'Locked'
    | 'NoRecycleBin'
    | 'VerifyFailed'
    | 'TrashFailed'
    | 'Occupied'
    | 'Interrupted'
    | 'InvalidPath'
    | 'Io'
    /** Undo of a `trash` operation: the original path is taken; the item stays in the Recycle Bin. */
    | 'RestoreOccupied';

export interface OpError {
    code: FailCode;
    detail: string | null;
}

/** `valet_core::executor::undo::ConfirmReason`, serialized as the variant name. */
export type ConfirmReason = 'Modified' | 'SourceOccupied' | 'OriginalMissing';

export interface NeedsConfirmation {
    op_id: number;
    source: string;
    dest: string | null;
    reasons: ConfirmReason[];
}

export interface UndoFailure {
    op_id: number;
    source: string;
    dest: string | null;
    error: OpError;
}

export interface SkippedOp {
    op_id: number;
    state: OpState;
}

export interface Restored {
    op_id: number;
    path: string;
}

export interface UndoReport {
    batch_id: string;
    undo_batch_id: string | null;
    /** Ids of original operations now rolled back. */
    undone: number[];
    restored_as: Restored[];
    needs_confirmation: NeedsConfirmation[];
    failed: UndoFailure[];
    skipped: SkippedOp[];
    /**
     * Trashed files restored from the Recycle Bin whose content differs from
     * the hash taken before trashing. Also listed in `undone`.
     */
    restored_modified: Restored[];
}

export function dryRun(): Promise<DryRunResponse> {
    return invoke<DryRunResponse>('dry_run');
}

/** The user left the Preview without executing it; the stored plan is dropped. */
export function closePreview(): Promise<void> {
    return invoke<void>('close_preview');
}

/** Runs the plan returned by the last `dryRun`. Rejects with `PREVIEW_REQUIRED` otherwise. */
export function executeOperations(planId: string): Promise<ExecuteResponse> {
    return invoke<ExecuteResponse>('execute_operations', { planId });
}

/** `list_batches` result (docs/LICENSING.md). */
export interface BatchList {
    batches: BatchInfo[];
    /** The Free plan hid batches older than 7 days (they stay in the journal). */
    hidden_older: boolean;
    /**
     * Newest batch of kind `apply` in the whole journal, even if already
     * undone; the only batch the Free plan can undo. Null when there is none.
     */
    latest_apply_batch_id: string | null;
}

function firstApplyId(batches: BatchInfo[]): string | null {
    return batches.find((b) => b.kind === 'apply')?.id ?? null;
}

/**
 * Newest first. The backend default limit is 50. A plain array (backends
 * before the licensing contract) is read as a list with no plan limits.
 */
export async function listBatches(limit?: number): Promise<BatchList> {
    const raw = await invoke<unknown>('list_batches', { limit: limit ?? null });
    if (Array.isArray(raw)) {
        const batches = raw as BatchInfo[];
        return { batches, hidden_older: false, latest_apply_batch_id: firstApplyId(batches) };
    }
    if (typeof raw === 'object' && raw !== null) {
        const r = raw as { batches?: unknown; hidden_older?: unknown; latest_apply_batch_id?: unknown };
        const batches = Array.isArray(r.batches) ? (r.batches as BatchInfo[]) : [];
        return {
            batches,
            hidden_older: r.hidden_older === true,
            latest_apply_batch_id:
                typeof r.latest_apply_batch_id === 'string'
                    ? r.latest_apply_batch_id
                    : r.latest_apply_batch_id === null
                      ? null
                      : firstApplyId(batches),
        };
    }
    return { batches: [], hidden_older: false, latest_apply_batch_id: null };
}

export function getBatch(batchId: string): Promise<BatchDetail> {
    return invoke<BatchDetail>('get_batch', { batchId });
}

export function undoBatch(batchId: string, force: boolean): Promise<UndoReport> {
    return invoke<UndoReport>('undo_batch', { batchId, force });
}

export type Language = 'ro' | 'en';

/** Updater channels `save_app_settings` accepts; anything else is `settings.invalidChannel`. */
export const UPDATE_CHANNELS = ['stable', 'beta'] as const;
export type UpdateChannel = (typeof UPDATE_CHANNELS)[number];

/** `app_settings.json`, settings v2. */
export interface AppSettings {
    language: Language;
    notifications_enabled: boolean;
    auto_start: boolean;
    /** Takes effect only once `first_preview_approved` is true. */
    auto_apply_enabled: boolean;
    /** Set by the backend after the user approves a preview. Read-only here. */
    first_preview_approved: boolean;
    onboarding_done: boolean;
    /** Always set by `get_app_settings` and `save_app_settings`. */
    update_channel: UpdateChannel;
    /** DEBUG-level log (may contain paths; stays on this computer). */
    debug_logging: boolean;
    /** Daily automatic update check (default true). "Check now" works either way. */
    auto_update_check: boolean;
}

/** `config.json`. Only `inbox_paths` is edited by the UI; other fields are sent back unchanged. */
export interface Config {
    /** Watched folders. */
    inbox_paths: string[];
    [field: string]: unknown;
}

export function getAppSettings(): Promise<AppSettings> {
    return invoke<AppSettings>('get_app_settings');
}

export function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
    return invoke<AppSettings>('save_app_settings', { settings });
}

/** Why a `config.json` entry is refused. */
export type InvalidConfigCode = 'config.driveRoot' | 'config.excludedFolder';

/** A `config.json` entry (from a hand-edited file) that is not watched or planned. */
export interface InvalidConfigPath {
    path: string;
    code: InvalidConfigCode;
}

/**
 * What `get_config` returns: the stored config, every entry included in
 * `inbox_paths`, plus `invalid_paths` (refused; `saveConfig` rejects a list
 * that still contains one) and `unavailable_paths` (valid folders that do not
 * exist right now, e.g. an unplugged drive; saved, not watched until they
 * exist again).
 */
export interface ConfigView extends Config {
    invalid_paths: InvalidConfigPath[];
    unavailable_paths: string[];
    /** Free plan: folders beyond the first 2, kept in the config but not watched or planned. */
    inactive_paths?: string[];
}

export function getConfig(): Promise<ConfigView> {
    return invoke<ConfigView>('get_config');
}

/**
 * Rejects with a code such as `config.driveRoot` when a folder cannot be
 * watched. A folder that does not exist is saved (unavailable).
 */
export function saveConfig(config: Config): Promise<Config> {
    return invoke<Config>('save_config', { config });
}

export function configureAutoStart(enabled: boolean): Promise<void> {
    return invoke<void>('configure_auto_start', { enabled });
}

/** Whether the per-user auto-start entry exists and is enabled. */
export function getAutoStart(): Promise<boolean> {
    return invoke<boolean>('get_auto_start');
}

/** Error raised by a command: either a plain string or `{ code, detail }`. */
export interface CommandError {
    code: string;
    detail: string | null;
}

const CODE_RE = /^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)+$/;

/**
 * Splits a command error into a code and detail when it looks like one
 * (`config.driveRoot`, `config.driveRoot: D:\\`, or `{ code, detail }`).
 * Returns `null` for free text.
 */
export function parseCommandError(err: unknown): CommandError | null {
    if (typeof err === 'object' && err !== null && 'code' in err) {
        const { code, detail } = err as { code: unknown; detail?: unknown };
        if (typeof code === 'string') {
            return { code, detail: typeof detail === 'string' ? detail : null };
        }
    }
    if (typeof err !== 'string') {
        return null;
    }
    const sep = err.indexOf(': ');
    const code = sep === -1 ? err : err.slice(0, sep);
    if (!CODE_RE.test(code)) {
        return null;
    }
    return { code, detail: sep === -1 ? null : err.slice(sep + 2) };
}

/** Text of any rejected value, for messages that are not translated codes. */
export function errorMessage(err: unknown): string {
    if (typeof err === 'string') return err;
    if (err instanceof Error) return err.message;
    if (typeof err === 'object' && err !== null && 'message' in err) {
        const { message } = err as { message: unknown };
        if (typeof message === 'string') return message;
    }
    return String(err);
}

// ---------------------------------------------------------------- rules

/** Condition types `upsert_rule` accepts (A1 schema in `src-tauri/src/commands/rules.rs`). */
export const CONDITION_TYPES = [
    'ext',
    'extIn',
    'extNotIn',
    'typeGroup',
    'nameContains',
    'nameMatches',
    'pathContains',
    'sizeGt',
    'sizeLt',
    'modifiedOlderThanDays',
    'createdOlderThanDays',
    'emptyFolder',
    'duplicate',
] as const;
export type ConditionType = (typeof CONDITION_TYPES)[number];

export const TYPE_GROUPS = ['image', 'document', 'archive', 'installer', 'video', 'audio', 'code'] as const;
export type TypeGroup = (typeof TYPE_GROUPS)[number];

export const ACTION_TYPES = ['moveTo', 'copyTo', 'trash', 'report'] as const;
export type ActionType = (typeof ACTION_TYPES)[number];

export interface RuleCondition {
    type: string;
    value: unknown;
}

export interface RuleAction {
    type: string;
    /** `{ path }` for `moveTo`/`copyTo`; absent, null or `{}` otherwise. */
    params?: unknown;
}

/** `valet_core::rules::Rule` as JSON. */
export interface Rule {
    /** UUID; the UI generates it for new rules. */
    id: string;
    name: string;
    enabled: boolean;
    alwaysApply: boolean;
    conditions: RuleCondition[];
    actions: RuleAction[];
    /** Set by `upsert_rule`. */
    version: number;
    /** `includeHidden`, `topLevelOnly`; other keys are kept. Null when the rule has none. */
    options: Record<string, unknown> | null;
    /** Watched folders the rule applies to; empty: all. */
    scope: string[];
}

export function getRules(): Promise<Rule[]> {
    return invoke<Rule[]>('get_rules');
}

/** Validates and stores the rule. Rejects with a `rule.*` code. */
export function upsertRule(rule: Rule): Promise<Rule> {
    return invoke<Rule>('upsert_rule', { rule });
}

/** Removes the rule; no file is touched. Rejects with `rule.notFound` or `rule.storage`. */
export function deleteRule(ruleId: string): Promise<void> {
    return invoke<void>('delete_rule', { ruleId });
}

// ---------------------------------------------------------------- presets and onboarding

/** One shipped preset, localized by the saved language. */
export interface Preset {
    id: string;
    name: string;
    description: string;
    displayOrder: number;
    defaultChecked: boolean;
    defaultScope: string[];
    ruleCount: number;
    actions: ActionType[];
}

export function listPresets(): Promise<Preset[]> {
    return invoke<Preset[]>('list_presets');
}

export function installPresets(presetIds: string[]): Promise<Rule[]> {
    return invoke<Rule[]>('install_presets', { presetIds });
}

export interface OnboardingResult {
    config: Config;
    rules: Rule[];
}

/**
 * Saves the watched folders (tokens such as `{downloads}` are expanded by the
 * backend), installs the presets and marks onboarding done. Rejects with the
 * same `config.*` codes as `saveConfig`.
 */
export function completeOnboarding(folders: string[], presetIds: string[]): Promise<OnboardingResult> {
    return invoke<OnboardingResult>('complete_onboarding', { folders, presetIds });
}

// ---------------------------------------------------------------- updater

/** Result of `check_for_update`. `version`, `notes` and `date` come with `available: true`. */
export interface UpdateCheck {
    available: boolean;
    version?: string;
    notes?: string;
    date?: string;
    /** Publication date from the updater manifest. */
    pub_date?: string | null;
    /** Pro only: whether the license's `updates_until` covers the new build. Null when not Pro. */
    covered_by_license?: boolean | null;
}

/** Version of the running app. */
export function getAppVersion(): Promise<string> {
    return invoke<string>('get_app_version');
}

/**
 * Asks the update server of the saved channel for a newer version. Rejects
 * with `update.notConfigured` (builds without an updater), `update.network`
 * or `update.failed`.
 */
export function checkForUpdate(): Promise<UpdateCheck> {
    return invoke<UpdateCheck>('check_for_update');
}

/**
 * Downloads and installs the update, then restarts Valet. Rejects with
 * `update.busy` while a batch runs, `update.noUpdate`, `update.network`,
 * `update.notConfigured` or `update.failed`.
 */
export function installUpdate(): Promise<void> {
    return invoke<void>('install_update');
}

/** Payload of `update-progress`, sent while `installUpdate` downloads. Bytes. */
export interface UpdateProgress {
    downloaded: number;
    /** Null when the server does not send a size. */
    total: number | null;
}

/** Payload of `update-available`, sent by the backend's daily check. */
export interface UpdateAvailable {
    version: string;
}

export function onUpdateProgress(handler: (event: UpdateProgress) => void): Promise<UnlistenFn> {
    return listen<UpdateProgress>('update-progress', (e) => handler(e.payload));
}

export function onUpdateAvailable(handler: (event: UpdateAvailable) => void): Promise<UnlistenFn> {
    return listen<UpdateAvailable>('update-available', (e) => handler(e.payload));
}

// ---------------------------------------------------------------- events

/** Payload of the `batch-finished` event, sent after every batch (manual or auto-apply). */
export interface BatchFinished {
    batch_id: string;
    done: number;
    failed: number;
    /** True for batches the watcher ran (auto-apply). */
    auto: boolean;
}

export function onBatchFinished(handler: (event: BatchFinished) => void): Promise<UnlistenFn> {
    return listen<BatchFinished>('batch-finished', (e) => handler(e.payload));
}

// ---------------------------------------------------------------- plan and license (docs/LICENSING.md)

/** The plan in effect. Times are Unix seconds. */
export type PlanTier =
    | { kind: 'free' }
    | { kind: 'trial'; ends_at: number }
    | { kind: 'pro'; license_id: string; email: string; updates_until: number };

/** What the plan allows. `null` = unlimited. */
export interface PlanLimits {
    own_rules: number | null;
    folders: number | null;
    history_days: number | null;
    undo_latest_only: boolean;
    export_import: boolean;
}

export interface LicenseInfo {
    license_id: string;
    email: string;
    /** Unix seconds; builds published up to this date run as Pro. */
    updates_until: number;
}

/** Reason codes for not getting Pro; each has a sentence under `license.reason.<code>`. */
export const PLAN_REASONS = [
    'license.missing',
    'license.invalid_signature',
    'license.wrong_product',
    'license.not_covering_build',
    'activation.missing',
    'activation.other_machine',
    'activation.expired',
    'license.not_configured',
] as const;

/** Returned by `get_plan_status` and by every command that changes the plan. */
export interface PlanView {
    plan: PlanTier;
    /** Codes from `PLAN_REASONS`. */
    reasons: string[];
    limits: PlanLimits;
    /** `pubkey.txt` and `api.txt` are set: Pro can be activated. */
    licensing_configured: boolean;
    /** `checkout.txt` is set: buying is open. */
    checkout_available: boolean;
    /** Show the "pick what stays active" dialog. */
    choice_pending: boolean;
    /** The stored license, also when it does not give Pro (to explain why). */
    license: LicenseInfo | null;
    trial_ends_at: number | null;
    /** `checked_at` of the stored activation token. */
    last_check_at: number | null;
    /** Microsoft Store (MSIX) build: updates and auto-start are handled by Windows. */
    packaged: boolean;
}

/** Resolves the plan and applies a pending downgrade or upgrade. */
export function getPlanStatus(): Promise<PlanView> {
    return invoke<PlanView>('get_plan_status');
}

/** Native open dialog for a `.valet-license` file, then like `importLicenseText`. Null when cancelled. */
export function importLicenseFile(): Promise<PlanView | null> {
    return invoke<PlanView | null>('import_license_file');
}

/**
 * Verifies the license text, activates it online and stores it. Rejects with
 * `license.not_configured`, `license.invalid_signature`, `license.wrong_product`,
 * `license.not_covering_build`, `license.network`, `license.unknown`,
 * `license.too_many` or `license.revoked`.
 */
export function importLicenseText(text: string): Promise<PlanView> {
    return invoke<PlanView>('import_license_text', { text });
}

/** Online validation now, even before 30 days. Rejects with `license.network` or the codes above. */
export function checkLicenseNow(): Promise<PlanView> {
    return invoke<PlanView>('check_license_now');
}

/** Deactivates this computer (best effort) and moves the license files aside (never deleted). */
export function removeLicense(): Promise<PlanView> {
    return invoke<PlanView>('remove_license');
}

/**
 * Free only: these own rules (at most 3) stay enabled, these folders (at most 2)
 * move to the front of `inbox_paths`. Rejects with `plan.limit` or `plan.notFree`.
 */
export function applyPlanChoice(ruleIds: string[], folders: string[]): Promise<PlanView> {
    return invoke<PlanView>('apply_plan_choice', { ruleIds, folders });
}

/**
 * Pro/Trial: native save dialog, then writes the export. Resolves with the
 * written path, or null when the dialog was cancelled. Rejects with
 * `plan.proOnly`, `export.exists` or `export.io`.
 */
export async function exportRulesFile(): Promise<string | null> {
    return (await invoke<string | null>('export_rules_file')) ?? null;
}

/**
 * Pro/Trial: native open dialog, then imports the rules as new rules
 * (auto-apply waits for a preview). Resolves with the imported rules, or
 * null when the dialog was cancelled. Rejects with `plan.proOnly` or an
 * `import.*` code.
 */
export async function importRulesFile(): Promise<Rule[] | null> {
    return (await invoke<Rule[] | null>('import_rules_file')) ?? null;
}

/** What `open_external` can open: the checkout page or the Windows startup apps settings. */
export type ExternalTarget = 'checkout' | 'startup_settings';

/** Rejects with `open.unavailable`. */
export function openExternal(target: ExternalTarget): Promise<void> {
    return invoke<void>('open_external', { target });
}

/** Payload of `plan-changed`, sent when the plan changes in the background (hourly check, trial end). */
export function onPlanChanged(handler: (plan: PlanView) => void): Promise<UnlistenFn> {
    return listen<PlanView>('plan-changed', (e) => handler(e.payload));
}

/** Error code of a change refused by a Free plan limit. */
export const PLAN_LIMIT = 'plan.limit';

export interface PlanLimitError {
    kind: 'rules' | 'folders';
    limit: number;
}

/**
 * The limit behind a `plan.limit` error, sent as `{ code, kind, limit }` or
 * as `plan.limit: rules 3`. Null for other errors.
 */
export function planLimitOf(err: unknown): PlanLimitError | null {
    if (typeof err === 'object' && err !== null && (err as { code?: unknown }).code === PLAN_LIMIT) {
        const { kind, limit } = err as { kind?: unknown; limit?: unknown };
        const k = kind === 'folders' ? 'folders' : 'rules';
        return { kind: k, limit: typeof limit === 'number' ? limit : k === 'folders' ? 2 : 3 };
    }
    const parsed = parseCommandError(err);
    if (parsed?.code !== PLAN_LIMIT) return null;
    const m = /^(rules|folders)\s+(\d+)/.exec(parsed.detail ?? '');
    if (m) return { kind: m[1] as 'rules' | 'folders', limit: Number(m[2]) };
    const k = parsed.detail?.startsWith('folders') ? 'folders' : 'rules';
    return { kind: k, limit: k === 'folders' ? 2 : 3 };
}
