---
layout: home
title: "OpenFlow: run AI agents as a workflow"
description: Install OpenFlow, connect a model provider, and build multi-agent workflows.
hero:
  text: Run AI agents as a workflow
  tagline: Chain agents on a canvas. Each gets its own task, model, and tools, and passes its result to the next. You can read every conversation and step in at any point.
  image:
    src: /logo.png
    alt: ''
  actions:
    - theme: brand
      text: Install
      link: /getting-started/
    - theme: alt
      text: Build your first workflow
      link: /guides/first-workflow
    - theme: alt
      text: GitHub
      link: https://github.com/philbotar/OpenFlow
---

<dl class="home-points">
  <div><dt>Draw the workflow</dt><dd>Each step is an agent with a prompt, a model, and tools. Connect steps to say which results feed which.</dd></div>
  <div><dt>Run steps in parallel</dt><dd>Steps that don't depend on each other run at the same time. A step starts when everything it needs is ready.</dd></div>
  <div><dt>Stay in control</dt><dd>Open any agent's conversation to answer its questions, approve a tool call, or retry a failed step.</dd></div>
  <div><dt>Bring your own model</dt><dd>Use OpenAI, Anthropic, ChatGPT sign-in, Amazon Bedrock, OpenRouter, or a local model such as Ollama.</dd></div>
</dl>

## See it

[![OpenFlow canvas: a feature brief splits into implementation, tests, and risk review, then joins at a delivery plan.](public/media/screenshots/offline-search-canvas.png)](public/media/screenshots/offline-search-canvas.png)

One brief fans out to three agents that work at once. The delivery plan waits for all three.

## Get started

1. [Install OpenFlow](getting-started/README.md). The macOS app needs no development tools.
2. [Connect a provider](guides/provider-setup.md) so agents can call a model.
3. [Build your first workflow](guides/first-workflow.md), or [start a direct chat](guides/direct-chats.md) if you only want to talk to one agent.

## Find what you need

| I want to… | Read |
| --- | --- |
| Understand how it works | [How OpenFlow works](concepts/how-openflow-works.md) |
| Design a workflow | [Workflow authoring](guides/workflow-authoring.md) |
| Handle approvals, retries, and failures | [Runs and recovery](guides/runs.md) |
| Look up a setting | [Configuration](reference/configuration.md) |
| Fix a problem | [Troubleshooting](troubleshooting/README.md) |
| Build it from source | [Build from source](getting-started/build-from-source.md) |
| Browse everything | [All documentation](README.md) |
