import fs from 'node:fs'
import path from 'node:path'
import type { HeadConfig, PageData, SiteConfig } from 'vitepress'
import { docsRoot, repo, resolveLink, siteUrl } from './links'

const siteName = 'OpenFlow'
const siteDescription = 'Install OpenFlow, connect a model provider, and build multi-agent workflows.'
const shareImage = { path: 'media/workflow-demo-poster.png', width: '1280', height: '800' }

const root = siteUrl.replace(/\/$/, '')

const sections: Array<[string, string]> = [
  ['getting-started', 'Get started'],
  ['guides', 'Guides'],
  ['troubleshooting', 'Troubleshooting'],
  ['concepts', 'Concepts'],
  ['reference', 'Reference'],
  ['architecture', 'Architecture'],
  ['contributing', 'Contributing'],
]

/** Public URL path for a published page, relative to the site root (no leading slash). */
export function htmlPath(relativePath: string): string {
  if (relativePath === 'index.md') return ''
  return relativePath.replace(/(^|\/)index\.md$/, '$1').replace(/\.md$/, '.html')
}

/** Path of the Markdown export for a published page. */
function markdownPath(relativePath: string): string {
  return relativePath
}

function readSource(srcRelative: string): { frontmatter: Record<string, string>; body: string } {
  const raw = fs.readFileSync(path.join(docsRoot, srcRelative), 'utf8')
  const match = raw.match(/^---\n([\s\S]*?)\n---\n?/)
  const frontmatter: Record<string, string> = {}
  if (match) {
    for (const line of match[1].split('\n')) {
      const kv = line.match(/^([A-Za-z]+):\s*(.+)$/)
      if (kv) frontmatter[kv[1]] = kv[2].trim()
    }
  }
  return { frontmatter, body: match ? raw.slice(match[0].length) : raw }
}

function plain(text: string): string {
  return text
    .replace(/!\[[^\]]*]\([^)]*\)/g, '')
    .replace(/\[([^\]]+)]\([^)]*\)/g, '$1')
    .replace(/<[^>]+>/g, '')
    .replace(/[*_`]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}

/** First prose paragraph of a page, trimmed to a search-snippet length. */
export function describeBody(body: string): string | undefined {
  let inFence = false
  for (const block of body.split(/\n\s*\n/)) {
    const first = block.trimStart()
    if (/^(```|~~~)/.test(first)) inFence = !inFence && !/^(```|~~~)[\s\S]*\n(```|~~~)\s*$/.test(first.trim())
    if (inFence) continue
    if (/^(#|<|\||!|\[!\[|>|[-*+] |\d+\. |```|~~~|---)/.test(first)) continue
    const text = plain(block)
    if (text.length < 40) continue
    if (text.length <= 160) return text
    const cut = text.slice(0, 160)
    const sentence = cut.lastIndexOf('. ')
    if (sentence > 80) return cut.slice(0, sentence + 1)
    return `${cut.slice(0, cut.lastIndexOf(' '))}…`
  }
  return undefined
}

function titleOf(frontmatter: Record<string, string>, body: string, fallback: string): string {
  return frontmatter.title ?? body.match(/^#\s+(.+)$/m)?.[1].trim() ?? fallback
}

function sourceOf(siteConfig: SiteConfig, relativePath: string): string {
  return siteConfig.rewrites.inv[relativePath] ?? relativePath
}

/** Give every page its own description and avoid "OpenFlow | OpenFlow" titles. */
export function transformPage(pageData: PageData, siteConfig: SiteConfig): void {
  if (!pageData.frontmatter.description) {
    const { body } = readSource(sourceOf(siteConfig, pageData.relativePath))
    pageData.description = describeBody(body) ?? siteDescription
  }
  if (/openflow/i.test(pageData.title)) {
    pageData.titleTemplate = false
    pageData.frontmatter.titleTemplate = false
  }
}

export function pageHead(pageData: PageData): HeadConfig[] {
  const url = `${siteUrl}${htmlPath(pageData.relativePath)}`
  const title = pageData.title || siteName
  const description = pageData.description || siteDescription
  const image = `${siteUrl}${shareImage.path}`
  const head: HeadConfig[] = [
    ['link', { rel: 'canonical', href: url }],
    ['link', { rel: 'alternate', type: 'text/markdown', href: `${siteUrl}${markdownPath(pageData.relativePath)}` }],
    ['meta', { property: 'og:site_name', content: siteName }],
    ['meta', { property: 'og:type', content: pageData.relativePath === 'index.md' ? 'website' : 'article' }],
    ['meta', { property: 'og:title', content: title }],
    ['meta', { property: 'og:description', content: description }],
    ['meta', { property: 'og:url', content: url }],
    ['meta', { property: 'og:image', content: image }],
    ['meta', { property: 'og:image:width', content: shareImage.width }],
    ['meta', { property: 'og:image:height', content: shareImage.height }],
    ['meta', { property: 'og:image:alt', content: 'OpenFlow running a workflow with agents in parallel.' }],
  ]
  if (pageData.relativePath === 'index.md') {
    const app = {
      '@context': 'https://schema.org',
      '@type': 'SoftwareApplication',
      name: siteName,
      description: siteDescription,
      url: siteUrl,
      image,
      applicationCategory: 'DeveloperApplication',
      operatingSystem: 'macOS',
      downloadUrl: `${repo}/releases/latest`,
      license: 'https://opensource.org/licenses/MIT',
      codeRepository: repo,
      offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    }
    head.push(['script', { type: 'application/ld+json' }, JSON.stringify(app)])
  }
  return head
}

function toAbsoluteLinks(markdown: string, srcRelative: string): string {
  return markdown
    .replace(/^<a id="[^"]*"><\/a>\s*$/gm, '')
    .replace(/\]\(([^)\s]+)((?:\s+"[^"]*")?)\)/g, (whole, href: string, title: string) => {
      const resolved = resolveLink(href, srcRelative)
      if (!resolved) return whole
      return `](${resolved.startsWith('/') ? root + resolved : resolved}${title})`
    })
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

/** Write llms.txt, llms-full.txt, and one Markdown file per published page. */
export function writeLlmsFiles(siteConfig: SiteConfig): void {
  const out = siteConfig.outDir
  const entries = siteConfig.pages
    .filter((src) => src.endsWith('.md'))
    .map((src) => {
      const rewritten = siteConfig.rewrites.map[src] ?? src
      const { frontmatter, body } = readSource(src)
      const title = titleOf(frontmatter, body, rewritten)
      const description = frontmatter.description ?? describeBody(body) ?? ''
      return { src, rewritten, title, description, markdown: toAbsoluteLinks(body, src) }
    })
    .sort((a, b) => a.rewritten.localeCompare(b.rewritten))

  for (const page of entries) {
    const file = path.join(out, markdownPath(page.rewritten))
    fs.mkdirSync(path.dirname(file), { recursive: true })
    fs.writeFileSync(file, `# ${page.title}\n\n${page.markdown.replace(/^#\s+.+\n+/, '')}\n`)
  }

  const link = (p: (typeof entries)[number]) =>
    `- [${p.title}](${siteUrl}${markdownPath(p.rewritten)})${p.description ? `: ${p.description}` : ''}`
  const grouped = new Map<string, string[]>()
  for (const p of entries) {
    if (p.rewritten === 'index.md') continue
    const top = p.rewritten.includes('/') ? p.rewritten.split('/')[0] : ''
    const name = sections.find(([dir]) => dir === top)?.[1] ?? 'Overview'
    grouped.set(name, [...(grouped.get(name) ?? []), link(p)])
  }
  const order = ['Overview', ...sections.map(([, name]) => name)]
  const llms = [
    `# ${siteName}`,
    '',
    `> ${siteDescription} OpenFlow is a macOS desktop app where you connect AI agents on a canvas, give each a task, model, and tools, and run them as a workflow. Independent steps run in parallel, and you can read every conversation, answer questions, approve tool calls, and retry steps.`,
    '',
    `Source: ${repo}. Every page below is available as Markdown; the full text is in ${siteUrl}llms-full.txt.`,
    '',
    ...order.flatMap((name) => (grouped.has(name) ? [`## ${name}`, '', ...grouped.get(name)!, ''] : [])),
  ].join('\n')
  fs.writeFileSync(path.join(out, 'llms.txt'), `${llms.trimEnd()}\n`)

  const full = entries
    .filter((p) => p.rewritten !== 'index.md')
    .map((p) => `# ${p.title}\n\nURL: ${siteUrl}${htmlPath(p.rewritten)}\n\n${p.markdown.replace(/^#\s+.+\n+/, '')}`)
    .join('\n\n---\n\n')
  fs.writeFileSync(path.join(out, 'llms-full.txt'), `${full}\n`)
}
