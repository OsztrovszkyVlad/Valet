import React, { type ReactNode } from 'react';
import './Layout.css';

interface LayoutProps {
    children: ReactNode;
    activeView: string;
    onViewChange: (view: string) => void;
}

const Layout: React.FC<LayoutProps> = ({ children, activeView, onViewChange }) => {
    const views = [
        { id: 'rules', label: 'Rules', icon: '📁' },
        { id: 'preferences', label: 'Preferences', icon: '⚙️' },
        { id: 'dry-run', label: 'Dry Run', icon: '🔍' },
        { id: 'stats', label: 'Statistics', icon: '📊' }
    ];

    return (
        <div className="layout">
            {/* Header */}
            <header className="header">
                <div className="header-content">
                    <h1 className="app-title">
                        <span className="app-icon">🗃️</span>
                        Valet
                    </h1>
                    <p className="app-subtitle">Automated File Organization</p>
                </div>
            </header>

            {/* Navigation */}
            <nav className="nav">
                <div className="nav-tabs">
                    {views.map((view) => (
                        <button
                            key={view.id}
                            className={`nav-tab ${activeView === view.id ? 'active' : ''}`}
                            onClick={() => onViewChange(view.id)}
                        >
                            <span className="nav-icon">{view.icon}</span>
                            <span className="nav-label">{view.label}</span>
                        </button>
                    ))}
                </div>
            </nav>

            {/* Main Content */}
            <main className="main-content">
                {children}
            </main>

            {/* Footer */}
            <footer className="footer">
                <div className="footer-content">
                    <span className="status-indicator">
                        <span className="status-dot active"></span>
                        File watcher active
                    </span>
                </div>
            </footer>
        </div>
    );
};

export default Layout;
