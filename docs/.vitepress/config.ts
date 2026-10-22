import { defineConfig } from 'vitepress'

const siteUrl = process.env.DOCS_SITE_URL ?? 'https://philbotar.github.io/OpenFlow/'
const base = new URL(siteUrl).pathname
const repo = 'https://github.com/philbotar/OpenFlow'

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
