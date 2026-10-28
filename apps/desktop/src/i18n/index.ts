import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import ro from './ro.json';
import en from './en.json';

void i18n.use(initReactI18next).init({
    resources: {
        ro: { translation: ro },
        en: { translation: en },
    },
    lng: 'ro',
    fallbackLng: 'ro',
    interpolation: {
        // React escapes rendered text.
        escapeValue: false,
    },
});

export default i18n;
