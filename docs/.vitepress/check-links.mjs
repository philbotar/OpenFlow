// Verifies that internal links, images, and anchors in the built site resolve.
import fs from 'node:fs'
import path from 'node:path'

const dist = path.resolve(import.meta.dirname, 'dist')
const siteUrl = process.env.DOCS_SITE_URL ?? 'https://philbotar.github.io/OpenFlow/'
const base = new URL(siteUrl).pathname

const pages = fs
  .readdirSync(dist, { recursive: true })
  .map(String)
  .filter((file) => file.endsWith('.html') && file !== '404.html')

const idCache = new Map()
const idsOf = (file) => {
  if (!idCache.has(file)) {
    const html = fs.readFileSync(path.join(dist, file), 'utf8')
    idCache.set(file, new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])))
  }
  return idCache.get(file)
}

const problems = new Map()
const report = (page, target) => {
  if (!problems.has(target)) problems.set(target, new Set())
  problems.get(target).add(page)
}

for (const page of pages) {
  const html = fs.readFileSync(path.join(dist, page), 'utf8')
  const refs = [...html.matchAll(/<(?:a|img|source)\b[^>]*?\s(?:href|src)="([^"]*)"/g)].map((m) => m[1])
  for (const ref of refs) {
    if (!ref || /^(mailto:|data:|https?:|javascript:)/.test(ref)) continue
    const [rawPath, fragment] = ref.split('#')
    const clean = rawPath.split('?')[0]
    let target
    if (!clean) target = page
    else if (clean.startsWith(base)) target = clean.slice(base.length)
    else if (clean.startsWith('/')) {
      report(page, ref)
      continue
    } else target = path.normalize(path.join(path.dirname(page), clean))
    if (target === '' || target.endsWith('/')) target += 'index.html'
    const file = path.join(dist, target)
    if (fs.existsSync(file) && fs.statSync(file).isDirectory()) target = path.join(target, 'index.html')
    if (!fs.existsSync(path.join(dist, target))) report(page, ref)
    else if (fragment && target.endsWith('.html') && !idsOf(target).has(fragment)) report(page, ref)
  }
}

if (problems.size > 0) {
  for (const [target, from] of problems) console.error(`broken: ${target}  (in ${[...from][0]}${from.size > 1 ? ` +${from.size - 1}` : ''})`)
  process.exit(1)
}
console.log(`Checked ${pages.length} pages: all internal links, images, and anchors resolve.`)
