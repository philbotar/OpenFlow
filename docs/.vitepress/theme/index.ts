import DefaultTheme from 'vitepress/theme-without-fonts'
import { inBrowser, type Theme } from 'vitepress'
import { startAnalytics } from './analytics'
import Layout from './Layout.vue'
import Mermaid from './Mermaid.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app, router }) {
    app.component('Mermaid', Mermaid)
    if (inBrowser) startAnalytics(router)
  },
} satisfies Theme
