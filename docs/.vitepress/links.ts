import fs from 'node:fs'
import path from 'node:path'

export const siteUrl = process.env.DOCS_SITE_URL ?? 'https://philbotar.github.io/OpenFlow/'
export const base = new URL(siteUrl).pathname
export const repo = 'https://github.com/philbotar/OpenFlow'
export const docsRoot = path.resolve(import.meta.dirname, '..')
const repoRoot = path.resolve(docsRoot, '..')
const unpublished = new Set(['AGENTS.md', 'CONTEXT.md', 'ROADMAP.md'])

// Resolve a Markdown link the way the site publishes it: published pages get
// site URLs, everything else in the repository points at GitHub.
export function resolveLink(href: string, relativePath: string): string | undefined {
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
