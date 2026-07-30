import type { SkillSummary } from "../types";

export interface ChatSubmissionResolution {
  bodyText: string;
  invokedSkills: string[];
  submittedText: string;
}

export interface ActiveSlashToken {
  query: string;
  replaceStart: number;
  replaceEnd: number;
}

const SLASH_TOKEN_PATTERN = /(?:^|\s)(\/[^\s]*)$/;

export function getActiveSlashToken(input: string, caret: number): ActiveSlashToken | null {
  const safeCaret = Math.max(0, Math.min(caret, input.length));
  const beforeCaret = input.slice(0, safeCaret);
  const match = beforeCaret.match(SLASH_TOKEN_PATTERN);
  if (!match?.[1]) {
    return null;
  }

  const token = match[1];
  return {
    query: token.slice(1),
    replaceStart: safeCaret - token.length,
    replaceEnd: safeCaret,
  };
}

function skillMatchScore(skill: SkillSummary, query: string): number {
  if (query === "") {
    return 0;
  }

  const normalized = query.toLowerCase();
  const id = skill.id.toLowerCase();
  const name = skill.name.toLowerCase();
  if (id.startsWith(normalized)) {
    return 0;
  }
  if (name.startsWith(normalized)) {
    return 1;
  }
  if (id.includes(normalized)) {
    return 2;
  }
  if (name.includes(normalized)) {
    return 3;
  }
  return 4;
}

export function matchSkillsForSlashQuery(
  skills: readonly SkillSummary[],
  query: string,
  limit = 8,
): SkillSummary[] {
  const matches = skills
    .map((skill) => ({ skill, score: skillMatchScore(skill, query) }))
    .filter((entry) => entry.score < 4)
    .sort((left, right) => {
      if (left.score !== right.score) {
        return left.score - right.score;
      }
      return left.skill.id.localeCompare(right.skill.id);
    })
    .map((entry) => entry.skill);

  if (query === "") {
    return [...skills].sort((left, right) => left.id.localeCompare(right.id)).slice(0, limit);
  }

  return matches.slice(0, limit);
}

export function applySlashTokenCompletion(
  input: string,
  replaceStart: number,
  replaceEnd: number,
  skillId: string,
): { value: string; caret: number } {
  const value = `${input.slice(0, replaceStart)}/${skillId} ${input.slice(replaceEnd)}`;
  const caret = replaceStart + skillId.length + 2;
  return { value, caret };
}

export function resolveChatSubmission(
  input: string,
  knownSkillIds: ReadonlySet<string>,
): ChatSubmissionResolution {
  const trimmed = input.trim();
  if (trimmed === "") {
    return {
      bodyText: "",
      invokedSkills: [],
      submittedText: "",
    };
  }

  const invokedSkills: string[] = [];
  const bodyTokens: string[] = [];
  for (const token of trimmed.split(/\s+/)) {
    const skill = token.startsWith("/") ? token.slice(1) : "";
    if (skill !== "" && knownSkillIds.has(skill)) {
      if (!invokedSkills.includes(skill)) {
        invokedSkills.push(skill);
      }
      continue;
    }
    bodyTokens.push(token);
  }

  const bodyText = bodyTokens.join(" ");
  return {
    bodyText,
    invokedSkills,
    submittedText: bodyText,
  };
}
