import fs from 'node:fs'
import path from 'node:path'
import { slug } from 'github-slugger'
import { defineConfig } from 'vitepress'

const siteUrl = process.env.DOCS_SITE_URL ?? 'https://philbotar.github.io/OpenFlow/'
const base = new URL(siteUrl).pathname
const repo = 'https://github.com/philbotar/OpenFlow'
const docsRoot = path.resolve(import.meta.dirname, '..')
const repoRoot = path.resolve(docsRoot, '..')
const unpublished = new Set(['AGENTS.md', 'CONTEXT.md', 'ROADMAP.md'])

// Resolve a Markdown link the way the site publishes it: published pages get
// site URLs, everything else in the repository points at GitHub.
function resolveLink(href: string, relativePath: string): string | undefined {
  if (/^([a-z][a-z0-9+.-]*:|#|\/\/)/i.test(href)) return undefined
  const [target, hash = ''] = href.split('#')
  if (!target) return undefined
  const from = path.resolve(docsRoot, path.dirname(relativePath))
  const abs = path.resolve(from, target)
  const suffix = hash ? `#${hash}` : ''
  const inDocs = abs === docsRoot || abs.startsWith(docsRoot + path.sep)
  const rel = path.relative(docsRoot, abs).split(path.sep).join('/')
  if (inDocs && rel.startsWith('public/')) return `/${rel.slice('public/'.length)}${suffix}`
  const published =
    inDocs && abs.endsWith('.md') && !unpublished.has(path.basename(abs)) && !rel.startsWith('work-specs/')
  if (published) {
    if (rel === 'README.md') return `/overview.html${suffix}`
    if (rel === 'index.md') return `/${suffix}`
    if (path.basename(rel) === 'README.md') return `/${path.dirname(rel)}/${suffix}`
    return `/${rel.replace(/\.md$/, '.html')}${suffix}`
  }
  if (!fs.existsSync(abs)) return undefined
  const repoRel = path.relative(repoRoot, abs).split(path.sep).join('/')
  const kind = fs.statSync(abs).isDirectory() ? 'tree' : 'blob'
  return `${repo}/${kind}/main/${repoRel}${suffix}`
}

export default defineConfig({
  title: 'OpenFlow',
  description: 'Install OpenFlow, connect a provider, and build multi-agent workflows you can inspect and control.',
  base,
  cleanUrls: false,
  lastUpdated: false,
  ignoreDeadLinks: true,
  srcExclude: ['**/AGENTS.md', '**/CONTEXT.md', '**/ROADMAP.md', 'work-specs/**', 'node_modules/**'],
  rewrites: {
    'README.md': 'overview.md',
    ':dir/README.md': ':dir/index.md',
    ':a/:b/README.md': ':a/:b/index.md',
  },
  markdown: {
    // Match GitHub's heading anchors so the same Markdown links work in both places.
    anchor: { slugify: (text: string) => slug(text) },
    config(md) {
      md.core.ruler.push('resolve-repo-links', (state) => {
        const relativePath = (state.env as { relativePath?: string }).relativePath
        if (!relativePath) return
        for (const block of state.tokens) {
          for (const token of block.children ?? []) {
            if (token.type !== 'link_open') continue
            const href = token.attrGet('href')
            const resolved = href ? resolveLink(href, relativePath) : undefined
            if (resolved) token.attrSet('href', resolved)
          }
        }
      })
      const fence = md.renderer.rules.fence!
      md.renderer.rules.fence = (tokens, idx, options, env, self) => {
        const token = tokens[idx]
        if (token.info.trim() === 'mermaid') {
          return `<Mermaid code="${encodeURIComponent(token.content)}" />`
        }
        return fence(tokens, idx, options, env, self)
      }
    },
  },
  head: [['link', { rel: 'icon', href: `${base}logo.png` }]],
  sitemap: { hostname: siteUrl },
  themeConfig: {
    logo: '/logo.png',
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: repo }],
    editLink: { pattern: `${repo}/edit/main/docs/:path`, text: 'Edit this page on GitHub' },
    nav: [
      { text: 'Get started', link: '/getting-started/' },
      { text: 'Guides', link: '/guides/first-workflow' },
      { text: 'Reference', link: '/reference/configuration' },
      { text: 'Architecture', link: '/architecture/' },
    ],
    sidebar: [
      {
        text: 'Start here',
        items: [
          { text: 'Overview', link: '/overview' },
          { text: 'Install OpenFlow', link: '/getting-started/' },
          { text: 'Build from source', link: '/getting-started/build-from-source' },
          { text: 'Connect a provider', link: '/guides/provider-setup' },
          { text: 'Build your first workflow', link: '/guides/first-workflow' },
          { text: 'For new users', link: '/guides/for-new-users' },
        ],
      },
      {
        text: 'Guides',
        items: [
          { text: 'Using the app', link: '/guides/using-the-app' },
          { text: 'Direct chats', link: '/guides/direct-chats' },
          { text: 'Workflow authoring', link: '/guides/workflow-authoring' },
          { text: 'Runs and recovery', link: '/guides/runs' },
          { text: 'Projects and persistence', link: '/guides/projects-and-persistence' },
          { text: 'Tools and integrations', link: '/guides/tools-and-integrations' },
          { text: 'Scheduling', link: '/guides/scheduling' },
          { text: 'Troubleshooting', link: '/troubleshooting/' },
        ],
      },
      {
        text: 'Concepts',
        items: [
          { text: 'Concepts', link: '/concepts/' },
          { text: 'How OpenFlow works', link: '/concepts/how-openflow-works' },
          { text: 'Workflows and runs', link: '/concepts/workflows-and-runs' },
          { text: 'Glossary', link: '/glossary' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'Data locations', link: '/reference/' },
          { text: 'Configuration', link: '/reference/configuration' },
          { text: 'Provider profiles', link: '/reference/providers' },
          { text: 'Tools', link: '/reference/tools' },
          { text: 'Security and data', link: '/security-and-data' },
        ],
      },
      {
        text: 'Architecture',
        items: [
          { text: 'Architecture overview', link: '/architecture/' },
          { text: 'Technical overview', link: '/architecture/technical-overview' },
          { text: 'Contract', link: '/architecture/contract' },
          { text: 'End-to-end runtime', link: '/architecture/end-to-end-runtime' },
          { text: 'Orchestration layout', link: '/architecture/orchestration-layout' },
          { text: 'Provider adapters', link: '/architecture/provider-adapters' },
          { text: 'Run persistence', link: '/architecture/run-persistence' },
          { text: 'Threading and concurrency', link: '/architecture/threading-concurrency' },
          { text: 'Callable agents', link: '/architecture/callable-agents' },
          { text: 'Output repair', link: '/architecture/output-repair' },
          { text: 'Plan and execute', link: '/architecture/plan-execute' },
        ],
      },
      {
        text: 'Contributing',
        items: [
          { text: 'Contributing', link: '/contributing/' },
          { text: 'Development commands', link: '/contributing/commands' },
          { text: 'Coding patterns', link: '/contributing/coding-patterns' },
          { text: 'Development lanes', link: '/contributing/development-lanes' },
          { text: 'Testing workflows', link: '/contributing/testing-workflows' },
          { text: 'Build troubleshooting', link: '/contributing/build-troubleshooting' },
          { text: 'Releasing', link: '/contributing/releasing' },
          { text: 'Documentation website', link: '/contributing/documentation-site' },
        ],
      },
    ],
    footer: { message: 'Released under the MIT License.', copyright: 'OpenFlow' },
  },
})
