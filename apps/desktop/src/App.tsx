import { useEffect, useState } from 'react'
import Layout from './components/Layout'
import RulesView from './views/RulesView'
import SettingsView from './views/SettingsView'
import DryRunView, { type LastBatch } from './views/DryRunView'
import StatsView from './views/StatsView'
import HistoryView from './views/HistoryView'
import { listBatches } from './api/commands'
import './App.css'

function App() {
  const [activeView, setActiveView] = useState('rules')
  // The batch the undo button on the preview screen refers to. Kept here so it
  // stays visible across tab changes until undone or replaced by a new batch.
  const [lastBatch, setLastBatch] = useState<LastBatch | null>(null)

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

  const renderActiveView = () => {
    switch (activeView) {
      case 'rules':
        return <RulesView />
      case 'settings':
        return <SettingsView />
      case 'dry-run':
        return (
          <DryRunView
            lastBatch={lastBatch}
            onBatchRan={setLastBatch}
            onLastBatchClosed={(complete) => {
              if (lastBatch) clearIfUndone(lastBatch.id, complete)
            }}
          />
        )
      case 'history':
        return <HistoryView onBatchUndone={clearIfUndone} />
      case 'stats':
        return <StatsView />
      default:
        return <RulesView />
    }
  }

  return (
    <Layout activeView={activeView} onViewChange={setActiveView}>
      {renderActiveView()}
    </Layout>
  )
}

export default App
