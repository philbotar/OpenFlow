---
name: product-docs-writer
description: Create or revise concise, evidence-based documentation for the application in this repository. Use for getting-started guides, feature documentation, task-based how-tos, concepts, configuration, CLI or API reference, troubleshooting, docs-site navigation, and internal cross-linking.
disable-model-invocation: true
---

# Product docs writer

## Objective

Write documentation that helps a reader complete a task or understand the application's actual behaviour.

Use the repository, tests, schemas, commands, and running application as evidence. Match the existing docs website rather than imposing a new structure or voice. Cover what the reader needs, then stop.

## Non-negotiable rules

- Do not invent behaviour, options, defaults, commands, limits, UI labels, examples, links, or error messages.
- Do not describe planned, experimental, commented-out, or unreachable code as an available feature.
- Do not hide uncertainty behind vague wording. Verify it or report it as unresolved.
- Do not copy implementation details into user documentation unless they affect setup, behaviour, limits, security, or troubleshooting.
- Do not duplicate an explanation that already has a canonical page. Summarise only the immediate point and link to that page.
- Do not create an orphan page. Add it to the existing navigation or parent index and add an appropriate inbound link.
- Do not rewrite unrelated documentation.
- Preserve the repository's established terminology, spelling convention, frontmatter, Markdown or MDX syntax, components, and link style.
- State limitations, destructive actions, permissions, data persistence, and side effects plainly.
- Prefer a correct omission over a plausible fabrication.

## Workflow

### 1. Discover the documentation system

Before drafting:

1. Locate the documentation root, site configuration, navigation or sidebar configuration, package scripts, and contribution or style guidance.
2. Determine whether the site uses Markdown or MDX, required frontmatter, custom components, callout syntax, heading conventions, and link conventions.
3. Read representative existing pages:
   - the documentation landing page;
   - one getting-started page;
   - one task-based guide;
   - one reference page;
   - one troubleshooting page, when present.
4. Find the canonical pages related to the requested subject.
5. Identify the available docs checks, such as formatting, linting, type checking, link checking, or site builds.

Follow existing conventions unless they are broken. Report a broken convention rather than silently spreading it.

### 2. Establish the page's job

Define these before writing:

- **Audience:** who is reading;
- **Outcome:** what the reader will know or complete;
- **Prerequisites:** what must already be installed, configured, understood, or permitted;
- **Page type:** tutorial, how-to, explanation, reference, or troubleshooting;
- **Canonical related pages:** existing pages that own prerequisite or adjacent information;
- **Verification:** how the reader knows the task worked.

A page must have one primary job. Split pages that serve unrelated goals.

When the request is broad, such as “document the application,” first map the main user journeys and existing coverage. Organise documentation around those journeys rather than source-code modules. Prefer a small connected page set over one large page.

### 3. Trace the implemented behaviour

Inspect enough of the application to document the feature accurately. Follow the user-facing path from its entry point through the relevant implementation.

Check, as applicable:

- visible UI labels, routes, states, and validation;
- CLI commands and `--help` output;
- API routes, request and response schemas, status codes, and errors;
- configuration schemas, defaults, environment variables, and precedence rules;
- permissions, authentication, feature flags, and platform restrictions;
- persistence, generated files, network calls, side effects, and destructive operations;
- tests and fixtures that demonstrate expected behaviour and edge cases;
- sample configuration and existing documentation.

Use this evidence priority:

1. Executable behaviour and current source code;
2. Tests, schemas, and generated command help;
3. Existing documentation and maintained examples;
4. Comments, issues, plans, and design notes.

When sources disagree, document the implemented behaviour and report the discrepancy. Do not silently choose the more convenient version.

### 4. Reuse the existing information architecture

Before adding a page, determine whether the material belongs in an existing canonical page.

For a new page:

- place it beside related pages;
- use the existing naming and slug conventions;
- add it to the correct sidebar, navigation group, or parent index;
- add at least one useful inbound link from an existing page;
- add prerequisite and next-step links where they help the reader continue;
- avoid adding the same link repeatedly within a section.

Do not create a new top-level category for a single page unless the current information architecture requires it.

### 5. Draft by page type

Use only the sections needed for the page. Do not add empty or ceremonial sections.

#### Tutorial

- Give one complete working path.
- Minimise choices and optional branches.
- Explain only the concepts required to finish.
- End with a verifiable result and a sensible next step.

#### How-to guide

- Start with the task outcome.
- State prerequisites briefly.
- Use ordered steps with exact controls, commands, paths, or values.
- Put alternatives under a short **Options** section rather than interrupting the main path.
- Include verification and likely failure cases.

#### Explanation

- Explain the model, lifecycle, or trade-off that affects user decisions.
- Use concrete application behaviour and examples.
- Do not turn the page into a procedure or source-code tour.

#### Reference

- Be complete, exact, and easy to scan.
- Include types, accepted values, defaults, required status, precedence, effects, errors, and examples where relevant.
- Use tables only when they make structured facts easier to compare.
- Separate normative behaviour from examples.

#### Troubleshooting

Organise entries as:

1. **Symptom** — what the reader observes;
2. **Likely cause** — the verified condition that produces it;
3. **Fix** — the shortest safe correction;
4. **Verify** — how to confirm recovery.

Do not list speculative causes as facts.

## Coverage checklist

Check each item for relevance. Include the information, not necessarily a heading.

- Reader and intended outcome;
- prerequisites and permissions;
- entry point in the UI, CLI, or API;
- required inputs and defaults;
- main procedure or behaviour;
- expected result and verification;
- persisted data, generated files, side effects, or destructive actions;
- common errors and recovery;
- limits, unsupported cases, platform differences, or feature flags;
- security, privacy, credentials, or network behaviour;
- links to prerequisite, conceptual, reference, troubleshooting, and next-step pages.

Coverage does not mean repeating every implementation detail. Include information that changes what the reader does, expects, or diagnoses.

## Cross-reference rules

Build a small index of existing documentation titles and paths before linking.

Link when the target page:

- is a prerequisite;
- owns the full explanation of a concept mentioned briefly here;
- contains the complete reference for an option, field, command, or API;
- is the likely next task;
- contains troubleshooting that would otherwise interrupt the current page.

Cross-links must:

- point to an existing file and valid heading anchor;
- use the site's established relative or absolute link convention;
- use descriptive link text, never “click here,” “learn more,” or a bare URL;
- appear where the relationship becomes useful, not only in a link dump;
- avoid sending the reader in a circle;
- avoid linking every occurrence of the same term.

A **Related pages** section is optional. Use it only when it adds value, and limit it to four links.

If a required canonical page is missing, create it only when it is within the requested scope. Otherwise, report the gap instead of inventing a link.

## Writing style

### Voice

- Use direct, neutral, technical language.
- Use active voice and concrete verbs.
- Use present tense for current behaviour.
- Address the reader as “you” only when it makes an instruction clearer.
- Never use “we” to refer to the product, company, documentation team, or reader.
- Use exact names from the interface, source, schemas, and commands.
- State the recommended path first. Put alternatives later.
- State the reason for a step only when it affects a decision, prevents an error, or explains an unexpected result.

### Structure

- Use sentence-case headings.
- Start with the result or purpose, not a generic introduction.
- Keep paragraphs to one idea and usually no more than three sentences.
- Prefer short paragraphs over dense bullet lists.
- Use numbered lists only for ordered actions.
- Use bullets for genuine sets of choices, requirements, or facts.
- Keep code examples minimal, complete, runnable, and consistent with the repository.
- Explain every placeholder in commands and examples.
- Use notes and warnings sparingly. Reserve warnings for destructive, irreversible, security-sensitive, or commonly misunderstood behaviour.
- Do not end with a summary that repeats the page.

### Concision

- Remove sentences that do not change what the reader understands or does.
- Do not repeat the title in the first sentence.
- Do not define standard technical terms unless the application gives them a specific meaning.
- Prefer one canonical example over several near-identical examples.
- Link to shared setup or concepts instead of restating them.
- Keep the main path uninterrupted; move edge cases to the relevant section.
- Do not add a conclusion merely to make the page feel finished.

## Anti–AI-speak rules

Do not use marketing language, filler, fake enthusiasm, or generic transitions.

Avoid these words and phrases unless they appear in a literal UI label or quotation:

- delve;
- leverage;
- utilise or utilize;
- seamless or seamlessly;
- robust;
- powerful;
- comprehensive;
- intuitive;
- streamline;
- unlock;
- supercharge;
- game-changing;
- effortlessly;
- “in today's ...”;
- “whether you're ...”;
- “at its core”;
- “it's important to note”;
- “in this guide, we'll ...”;
- “this section explores ...”;
- “by following these steps ...”;
- “this allows you to ...”;
- “designed to ...”;
- “ensures that ...”;
- “to get started, simply ...”;
- “simply,” “just,” or “easily” when they minimise a real step.

Avoid empty claims such as:

- “provides a flexible solution”;
- “improves the user experience”;
- “makes it easy to manage”;
- “offers a range of capabilities”;
- “helps users work more efficiently.”

Replace them with the exact action or effect.

Examples:

- Bad: “This powerful feature allows you to seamlessly manage providers.”
- Good: “Add, disable, and reorder providers from **Settings > Providers**.”

- Bad: “To get started, simply create a new workflow.”
- Good: “Select **New workflow**, then enter a name.”

- Bad: “It is important to note that deleting a run cannot be undone.”
- Good: “Deleting a run is permanent.”

- Bad: “The application leverages Git to provide robust version history.”
- Good: “The application stores each saved revision in Git.”

## Terminology rules

- Use one term for each concept across all affected pages.
- Prefer the current UI label unless the documentation has a clearly established canonical term.
- Introduce abbreviations once, then use them consistently.
- Do not alternate between near-synonyms such as “job,” “run,” “execution,” and “task” unless they are different domain concepts.
- Preserve exact casing for product names, commands, environment variables, configuration keys, and file paths.
- When renaming a term, search the documentation set and update affected references within scope.

## Examples and code

- Use examples that match real schemas and supported workflows.
- Use safe placeholder values. Never expose real secrets, tokens, personal data, production hosts, or private identifiers.
- Do not show an option that the application ignores or rejects.
- Prefer commands that can be copied without editing beyond clearly marked placeholders.
- State the working directory when it is not obvious.
- Include expected output only when verified. Do not fabricate terminal output.
- For APIs, show the smallest useful request and response, including relevant error behaviour.
- For configuration, document type, required status, default, accepted values, precedence, and restart requirements when applicable.

## Images and UI references

- Use screenshots only when spatial context materially helps the task.
- Do not use screenshots as a substitute for naming controls and steps.
- Use meaningful alt text.
- Do not rely on colour alone to explain status.
- Avoid screenshots of volatile values or private data.

## Validation

After editing:

1. Read the page as the intended user and confirm the primary task is clear in the first screen.
2. Check every factual statement against the implementation or an existing canonical source.
3. Check commands, paths, option names, defaults, examples, UI labels, and error messages.
4. Check all new and changed links, including heading anchors.
5. Confirm each new page is discoverable through navigation or a parent page and has an appropriate inbound link.
6. Search the draft for the anti–AI-speak terms and rewrite any matches that are not literal names.
7. Remove repetition, filler introductions, unnecessary conclusions, and duplicated canonical content.
8. Run the repository's documentation format, lint, type, link, and build checks when available.
9. Inspect the final diff for unrelated changes and broken formatting.
10. Do not leave placeholders or TODO markers in publishable documentation unless the user explicitly requested a draft.

If a check cannot run, state why. Do not claim validation that did not occur.

## Completion response

Report only:

- files created or changed;
- the user journeys or behaviours covered;
- significant cross-links or navigation changes;
- checks run and their results;
- implementation/documentation conflicts or facts that remain unverified.

Keep the response factual. Do not praise the result or repeat the documentation.
