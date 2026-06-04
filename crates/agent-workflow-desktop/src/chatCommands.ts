export const CHAT_SKILL_COMMANDS = [
  "brainstorming",
  "test-driven-development",
  "systematic-debugging",
  "writing-plans",
  "executing-plans",
  "verification-before-completion",
  "requesting-code-review",
  "receiving-code-review",
  "github:gh-fix-ci",
  "github:gh-address-comments",
  "browser",
  "documents",
] as const;

const CHAT_SKILL_COMMAND_LOOKUP: Record<string, true> = {
  brainstorming: true,
  "test-driven-development": true,
  "systematic-debugging": true,
  "writing-plans": true,
  "executing-plans": true,
  "verification-before-completion": true,
  "requesting-code-review": true,
  "receiving-code-review": true,
  "github:gh-fix-ci": true,
  "github:gh-address-comments": true,
  browser: true,
  documents: true,
};

export interface ChatSubmissionResolution {
  bodyText: string;
  invokedSkills: string[];
  submittedText: string;
}

export function resolveChatSubmission(input: string): ChatSubmissionResolution {
  const trimmed = input.trim();
  if (trimmed === "") {
    return {
      bodyText: "",
      invokedSkills: [],
      submittedText: "",
    };
  }

  const invokedSkills: string[] = [];
  let remaining = trimmed;

  while (remaining.startsWith("/")) {
    const separatorIndex = remaining.search(/\s/);
    const token = separatorIndex === -1 ? remaining : remaining.slice(0, separatorIndex);
    const skill = token.slice(1);
    if (skill === "" || CHAT_SKILL_COMMAND_LOOKUP[skill] !== true) {
      break;
    }
    if (!invokedSkills.includes(skill)) {
      invokedSkills.push(skill);
    }
    remaining = separatorIndex === -1 ? "" : remaining.slice(separatorIndex).trimStart();
  }

  const bodyText = remaining.trim();
  return {
    bodyText,
    invokedSkills,
    submittedText: formatSubmittedText(invokedSkills, bodyText, trimmed),
  };
}

function formatSubmittedText(invokedSkills: readonly string[], bodyText: string, fallbackText: string) {
  if (invokedSkills.length === 0) {
    return fallbackText;
  }

  const lines = ["Skill invocation:", ...invokedSkills.map((skill) => `- ${skill}`)];
  if (bodyText !== "") {
    lines.push("", "User message:", bodyText);
  }
  return lines.join("\n");
}
