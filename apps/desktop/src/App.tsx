import { useState } from 'react'
import Layout from './components/Layout'
import RulesView from './views/RulesView'
import PreferencesView from './views/PreferencesView'
import DryRunView from './views/DryRunView'
import StatsView from './views/StatsView'
import './App.css'

function App() {
  const [activeView, setActiveView] = useState('rules')

  const renderActiveView = () => {
    switch (activeView) {
      case 'rules':
        return <RulesView />
      case 'preferences':
        return <PreferencesView />
      case 'dry-run':
        return <DryRunView />
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
