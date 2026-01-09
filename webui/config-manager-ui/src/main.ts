import { createApp } from 'vue';
import { createI18n } from 'vue-i18n';
import App from './App.vue';
import './styles.css';
import zhCN from './locales/zh-CN.json';
import enUS from './locales/en-US.json';

const i18n = createI18n({
    legacy: false,
    locale: 'zh-CN',
    fallbackLocale: 'zh-CN',
    messages: {
        'zh-CN': zhCN,
        'en-US': enUS
    }
});

createApp(App).use(i18n).mount('#app');
