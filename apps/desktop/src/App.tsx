import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import Layout, { type View } from './components/Layout'
import LastBatchBox, { type LastBatch } from './components/LastBatchBox'
import RulesView from './views/RulesView'
import SettingsView from './views/SettingsView'
import DryRunView, { type InitialPreview } from './views/DryRunView'
import HistoryView from './views/HistoryView'
import OnboardingView from './views/OnboardingView'
import PlanBadge from './components/PlanBadge'
import PlanChoiceDialog from './components/PlanChoiceDialog'
import {
  closePreview,
  errorMessage,
  getAppSettings,
  getPlanStatus,
  listBatches,
  onBatchFinished,
  onPlanChanged,
  onUpdateAvailable,
  type PlanView,
} from './api/commands'
import './App.css'

type Phase = 'loading' | 'onboarding' | 'main'

function App() {
  const { t, i18n } = useTranslation()
  const [phase, setPhase] = useState<Phase>('loading')
  const [activeView, setActiveView] = useState<View>('preview')
  // The batch the undo box refers to. Kept here so it stays visible on every
  // screen until undone or replaced by a new batch.
  const [lastBatch, setLastBatch] = useState<LastBatch | null>(null)
  const [batchRunning, setBatchRunning] = useState(false)
  // Bumped when the journal changes outside the History screen.
  const [journalVersion, setJournalVersion] = useState(0)
  // The preview onboarding ran, shown once on the Preview screen.
  const [initialPreview, setInitialPreview] = useState<InitialPreview | null>(null)
  // Plan id of the preview waiting for approval, if any.
  const pendingPlan = useRef<string | null>(null)
  // Version announced by the backend's daily update check.
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [updateNoteDismissed, setUpdateNoteDismissed] = useState(false)
  // Null until `get_plan_status` answers (or when it failed): no plan limits are mirrored.
  const [plan, setPlan] = useState<PlanView | null>(null)
  // "Decide later" in the plan choice dialog; it comes back on the next start or plan change.
  const [choiceDeferred, setChoiceDeferred] = useState(false)
  // Bumped to scroll Settings to the Valet Pro section.
  const [proFocus, setProFocus] = useState(0)

  useEffect(() => {
    getAppSettings()
      .then((s) => {
        if (s.language !== i18n.language) void i18n.changeLanguage(s.language)
        setPhase(s.onboarding_done ? 'main' : 'onboarding')
      })
      .catch((err) => {
        console.error('Failed to read app settings:', err)
        setPhase('main')
      })
  }, [i18n])

  useEffect(() => {
    // After a restart, offer undo for the newest apply batch that still has done operations.
    listBatches(20)
      .then(({ batches }) => {
        const latest = batches.find((b) => b.kind === 'apply')
        if (latest && latest.finished_at !== null && latest.counts.done > 0) {
          setLastBatch((prev) => prev ?? { id: latest.id, createdAt: latest.created_at })
        }
      })
      .catch((err) => console.error('Failed to list batches:', err))
  }, [])

  useEffect(() => {
    // Batches from the Preview and from auto-apply: refresh History and the undo box.
    let unlisten: (() => void) | null = null
    let stopped = false
    onBatchFinished((e) => {
      setJournalVersion((v) => v + 1)
      if (e.done > 0) {
        setLastBatch({ id: e.batch_id, createdAt: new Date().toISOString(), auto: e.auto })
      }
    })
      .then((fn) => {
        if (stopped) fn()
        else unlisten = fn
      })
      .catch((err) => console.error('Failed to listen for batches:', errorMessage(err)))
    return () => {
      stopped = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    getPlanStatus()
      .then((p) => {
        if (!cancelled) setPlan(p)
      })
      .catch((err) => console.error('Failed to read the plan:', errorMessage(err)))
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    // Trial end, hourly license check, upgrade: the plan changed in the background.
    let unlisten: (() => void) | null = null
    let stopped = false
    onPlanChanged((p) => {
      setPlan(p)
      if (p.choice_pending) setChoiceDeferred(false)
    })
      .then((fn) => {
        if (stopped) fn()
        else unlisten = fn
      })
      .catch((err) => console.error('Failed to listen for plan changes:', errorMessage(err)))
    return () => {
      stopped = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | null = null
    let stopped = false
    onUpdateAvailable((e) => {
      setUpdateVersion(e.version)
      setUpdateNoteDismissed(false)
    })
      .then((fn) => {
        if (stopped) fn()
        else unlisten = fn
      })
      .catch((err) => console.error('Failed to listen for updates:', errorMessage(err)))
    return () => {
      stopped = true
      unlisten?.()
    }
  }, [])

  const changeView = (view: View) => {
    if (activeView === 'preview' && view !== 'preview') {
      if (pendingPlan.current !== null) {
        // Leaving the Preview without executing it.
        pendingPlan.current = null
        closePreview().catch((err) => console.error('close_preview failed:', errorMessage(err)))
      }
      setInitialPreview(null)
    }
    setActiveView(view)
  }

  const openPro = () => {
    changeView('settings')
    setProFocus((n) => n + 1)
  }

  const onPlanChange = useCallback((planId: string | null) => {
    pendingPlan.current = planId
  }, [])

  const clearIfUndone = (batchId: string, complete: boolean) => {
    if (complete) {
      setLastBatch((prev) => (prev && prev.id === batchId ? null : prev))
    }
  }

  const onBatchRan = (batch: LastBatch | null) => {
    setLastBatch(batch)
    setJournalVersion((v) => v + 1)
  }

  const onOnboardingFinished = (result: InitialPreview) => {
    setInitialPreview(result)
    setActiveView('preview')
    setPhase('main')
  }

  if (phase === 'loading') {
    return <p className="app-loading">{t('common.loading')}</p>
  }

  if (phase === 'onboarding') {
    return <OnboardingView onFinished={onOnboardingFinished} />
  }

  const renderActiveView = () => {
    switch (activeView) {
      case 'preview':
        return (
          <DryRunView
            onBatchRan={onBatchRan}
            onBusyChange={setBatchRunning}
            initial={initialPreview}
            intro={initialPreview !== null}
            onPlanChange={onPlanChange}
            onOpenSettings={() => changeView('settings')}
          />
        )
      case 'rules':
        return <RulesView plan={plan} onOpenPro={openPro} />
      case 'history':
        return <HistoryView onBatchUndone={clearIfUndone} reloadToken={journalVersion} plan={plan} />
      case 'settings':
        return (
          <SettingsView
            onOpenPreview={() => changeView('preview')}
            announcedVersion={updateVersion}
            plan={plan}
            onPlanChange={setPlan}
            proFocusToken={proFocus}
          />
        )
    }
  }

  // Main screens only; Settings shows the update itself.
  const updateNote = updateVersion !== null && !updateNoteDismissed && activeView !== 'settings' && (
    <div className="notice-message update-note" role="note" data-testid="update-note">
      <span>{t('updateNote.text', { version: updateVersion })}</span>
      <button className="btn-secondary" onClick={() => changeView('settings')}>
        {t('updateNote.open')}
      </button>
      <button className="error-close" onClick={() => setUpdateNoteDismissed(true)} aria-label={t('common.close')}>
        ×
      </button>
    </div>
  )

  const banner = (
    <>
      {updateNote}
      {lastBatch && (
        <LastBatchBox
          batch={lastBatch}
          canUndo={!batchRunning}
          onUndoRan={() => setJournalVersion((v) => v + 1)}
          onClose={(complete) => clearIfUndone(lastBatch.id, complete)}
        />
      )}
    </>
  )

  const showChoice = plan !== null && plan.choice_pending && !choiceDeferred

  return (
    <>
      <Layout
        activeView={activeView}
        onViewChange={changeView}
        banner={banner}
        updateBadge={updateVersion !== null}
        planIndicator={plan !== null && <PlanBadge plan={plan} onOpenPro={openPro} />}
      >
        {renderActiveView()}
      </Layout>
      {showChoice && (
        <PlanChoiceDialog
          plan={plan}
          onDone={(p) => {
            setPlan(p)
            setChoiceDeferred(false)
          }}
          onLater={() => setChoiceDeferred(true)}
        />
      )}
    </>
  )
}

export default App
