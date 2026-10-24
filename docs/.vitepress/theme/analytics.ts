import type { Router } from 'vitepress'

const endpoint = 'https://openflow-docs.goatcounter.com/count'
const script = 'https://gc.zgo.at/count.js'

interface GoatCounter {
  count: (options: { path: string }) => void
}

declare global {
  interface Window {
    goatcounter?: GoatCounter
  }
}

const localHosts = new Set(['localhost', '127.0.0.1', '[::1]'])

/**
 * Count page views with GoatCounter (no cookies, no personal data). The docs are
 * a single-page app, so the script's own load-time count is disabled and each
 * route change is counted here instead.
 */
export function startAnalytics(router: Router): void {
  if (localHosts.has(location.hostname)) return

  let lastPath = ''
  const count = (path: string) => {
    const clean = path.split('#')[0]
    if (!window.goatcounter || clean === lastPath) return
    lastPath = clean
    window.goatcounter.count({ path: clean })
  }

  const tag = document.createElement('script')
  tag.async = true
  tag.src = script
  tag.dataset.goatcounter = endpoint
  tag.dataset.goatcounterSettings = JSON.stringify({ no_onload: true })
  tag.addEventListener('load', () => count(location.pathname))
  document.head.append(tag)

  router.onAfterRouteChange = (to) => count(to)
}
