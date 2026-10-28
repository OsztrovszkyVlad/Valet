import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import Layout, { type View } from './components/Layout'
import LastBatchBox, { type LastBatch } from './components/LastBatchBox'
import RulesView from './views/RulesView'
import SettingsView from './views/SettingsView'
import DryRunView from './views/DryRunView'
import HistoryView from './views/HistoryView'
import { getAppSettings, listBatches } from './api/commands'
import './App.css'

function App() {
  const { i18n } = useTranslation()
  const [activeView, setActiveView] = useState<View>('preview')
  // The batch the undo box refers to. Kept here so it stays visible on every
  // screen until undone or replaced by a new batch.
  const [lastBatch, setLastBatch] = useState<LastBatch | null>(null)
  const [batchRunning, setBatchRunning] = useState(false)
  // Bumped when the journal changes outside the History screen.
  const [journalVersion, setJournalVersion] = useState(0)

  useEffect(() => {
    getAppSettings()
      .then((s) => {
        if (s.language !== i18n.language) void i18n.changeLanguage(s.language)
      })
      .catch((err) => console.error('Failed to read app settings:', err))
  }, [i18n])

  useEffect(() => {
    // After a restart, offer undo for the newest apply batch that still has done operations.
    listBatches(20)
      .then((batches) => {
        const latest = batches.find((b) => b.kind === 'apply')
        if (latest && latest.finished_at !== null && latest.counts.done > 0) {
          setLastBatch((prev) => prev ?? { id: latest.id, createdAt: latest.created_at })
        }
      })
      .catch((err) => console.error('Failed to list batches:', err))
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

  const renderActiveView = () => {
    switch (activeView) {
      case 'preview':
        return <DryRunView onBatchRan={onBatchRan} onBusyChange={setBatchRunning} />
      case 'rules':
        return <RulesView />
      case 'history':
        return <HistoryView onBatchUndone={clearIfUndone} reloadToken={journalVersion} />
      case 'settings':
        return <SettingsView />
    }
  }

  const banner = lastBatch && (
    <LastBatchBox
      batch={lastBatch}
      canUndo={!batchRunning}
      onUndoRan={() => setJournalVersion((v) => v + 1)}
      onClose={(complete) => clearIfUndone(lastBatch.id, complete)}
    />
  )

  return (
    <Layout activeView={activeView} onViewChange={setActiveView} banner={banner}>
      {renderActiveView()}
    </Layout>
  )
}

export default App
