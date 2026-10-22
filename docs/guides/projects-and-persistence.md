# Projects and persistence

Register a repository folder as a project to keep its workflows, run metadata, and attachments together. Workflows and [direct chats](direct-chats.md) use the project's execution folder for file operations. Choose a folder you are willing to expose to file tools and MCP roots; [Security and data](../security-and-data.md) describes those boundaries.

Registration does not copy the repository. It creates `{project}/.flow/` and tests write access with a temporary create, write, rename, and delete probe.

## Add and remove a project

Use the **Projects** heading's `+` action or **Add Project…** in a project selector. OpenFlow opens the native **Select project folder** dialog, registers the folder, and defaults its execution directory to the folder itself.

To unregister a project, choose **Remove project** from its row menu and confirm. OpenFlow blocks removal while a project run is active. The repository, workflow files, and other `.flow` files stay on disk.

## Project workflows

Project workflows live at:

```text
{project}/.flow/workflows/{workflowId}.workflow.json
```

Use **New workflow** or **Create with AI** from a project row to create a project workflow.

**Copy from…** creates an independent project copy of an existing workflow. The copy receives a new workflow ID and a name ending in `copy`; later changes do not sync with the source.

When OpenFlow loads the catalog, project-discovered workflows are merged with app workflows. A project file wins if the same workflow ID exists in both sources.

## Run locations

Choose the registered project in a new chat's **Project** control before sending its first message.

<figure class="product-shot product-shot--detail" style="--shot-width: 625px">

[![Prepared chat composer with the disposable OpenFlow-docs-demo project selected on the left.](../public/media/screenshots/project-chat-composer-detail.png)](../public/media/screenshots/project-chat-prepared.png)

<figcaption>The selected project appears below the message. This demo request has not been sent, so project selection is still editable. Select the image for the complete window.</figcaption>
</figure>

Project workflow and chat runs use the selected project's configured execution directory, or the project root when no override is configured. Run metadata is stored under:

```text
{project}/.flow/runs/{runId}/
```

Before a project run continues, OpenFlow probes whether `.flow` is writable. App workflows and chats without a project use isolated app-managed folders instead.

[Runtime and persistence paths](../reference/README.md#runtime-and-persistence-paths) lists the app-managed locations. [Runs and recovery](runs.md) explains resuming stored work. For chat project selection, follow [Start or restore a chat](direct-chats.md#start-or-restore-a-chat).

## What remains on disk

Deleting a workflow permanently removes its app record and every linked project workflow file. Existing durable run directories are not deleted with the workflow.

Deleting a chat removes its metadata and quarantines then deletes its associated run directory, including run-owned attachment copies. If cleanup cannot finish, OpenFlow reports pending cleanup and retries later.

Do not manually edit `projects.json`, `workflows.json`, `chats.json`, or run checkpoints while the app is running.
