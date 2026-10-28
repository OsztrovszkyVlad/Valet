// Runs before each test file: Romanian UI strings and DOM cleanup between tests.
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';
import i18n from '../i18n';

afterEach(async () => {
    cleanup();
    if (i18n.language !== 'ro') {
        await i18n.changeLanguage('ro');
    }
});
