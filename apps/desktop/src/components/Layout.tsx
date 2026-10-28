import React, { type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import './Layout.css';

export type MainView = 'preview' | 'rules' | 'history';
export type View = MainView | 'settings';

const MAIN_VIEWS: MainView[] = ['preview', 'rules', 'history'];

interface LayoutProps {
    children: ReactNode;
    activeView: View;
    onViewChange: (view: View) => void;
    /** Shown above the active view on every screen, e.g. the last-batch undo box. */
    banner?: ReactNode;
}

const Layout: React.FC<LayoutProps> = ({ children, activeView, onViewChange, banner }) => {
    const { t } = useTranslation();

    return (
        <div className="layout">
            <header className="header">
                <div className="header-content">
                    <h1 className="app-title">{t('app.title')}</h1>
                    <nav className="nav-tabs" role="tablist" aria-label={t('nav.label')}>
                        {MAIN_VIEWS.map((view) => (
                            <button
                                key={view}
                                role="tab"
                                aria-selected={activeView === view}
                                className={`nav-tab ${activeView === view ? 'active' : ''}`}
                                onClick={() => onViewChange(view)}
                            >
                                {t(`nav.${view}`)}
                            </button>
                        ))}
                    </nav>
                    <button
                        className={`nav-settings ${activeView === 'settings' ? 'active' : ''}`}
                        aria-pressed={activeView === 'settings'}
                        onClick={() => onViewChange('settings')}
                    >
                        <span className="nav-settings-icon" aria-hidden="true">
                            ⚙
                        </span>
                        {t('nav.settings')}
                    </button>
                </div>
            </header>

            <main className="main-content">
                {banner}
                {children}
            </main>
        </div>
    );
};

export default Layout;
