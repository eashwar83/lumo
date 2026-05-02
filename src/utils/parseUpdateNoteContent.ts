export type UpdateNotePrompt = {
    version: string;
    note: string;
};

export type UpdateNoteBlock =
    | {
          type: "heading";
          text: string;
          level: number;
      }
    | {
          type: "paragraph";
          text: string;
      }
    | {
          type: "list";
          ordered: boolean;
          items: string[];
      };

const escapeRegExp = (value: string) =>
    value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const stripInlineMarkdown = (value: string) =>
    value
        .replace(/\*\*([^*]+)\*\*/g, "$1")
        .replace(/\*([^*]+)\*/g, "$1")
        .replace(/`([^`]+)`/g, "$1")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
        .trim();

export const parseUpdateNoteContent = (version: string, note: string) => {
    const trimmedNote = note.trim();
    const fallbackTitle = `Version ${version} is available`;
    if (!trimmedNote) {
        return {
            title: fallbackTitle,
            blocks: [] as UpdateNoteBlock[],
        };
    }

    const lines = trimmedNote.split(/\r?\n/);
    const firstContentIndex = lines.findIndex((line) => line.trim().length > 0);
    if (firstContentIndex < 0) {
        return {
            title: fallbackTitle,
            blocks: [] as UpdateNoteBlock[],
        };
    }

    const normalizedVersion = version.trim().replace(/^v/i, "");
    const markdownReleaseHeadingPattern = new RegExp(
        `^#{1,6}\\s+\\[?v?${escapeRegExp(normalizedVersion)}\\]?\\s*-\\s*(.+)$`,
        "i",
    );
    const plainReleaseHeadingPattern = new RegExp(
        `^(?:release|version)\\s+v?${escapeRegExp(normalizedVersion)}\\s*$`,
        "i",
    );
    const firstLine = lines[firstContentIndex].trim();
    const markdownHeadingMatch = firstLine.match(markdownReleaseHeadingPattern);
    const title = markdownHeadingMatch
        ? `Version ${version} Released - ${markdownHeadingMatch[1].trim()}`
        : fallbackTitle;

    if (markdownHeadingMatch || plainReleaseHeadingPattern.test(firstLine)) {
        lines.splice(firstContentIndex, 1);
    }

    const blocks: UpdateNoteBlock[] = [];
    let paragraphLines: string[] = [];
    let listBlock: Extract<UpdateNoteBlock, { type: "list" }> | null = null;

    const flushParagraph = () => {
        const text = stripInlineMarkdown(paragraphLines.join(" "));
        paragraphLines = [];
        if (text) {
            blocks.push({
                type: "paragraph",
                text,
            });
        }
    };

    const flushList = () => {
        if (listBlock?.items.length) {
            blocks.push(listBlock);
        }
        listBlock = null;
    };

    for (const rawLine of lines) {
        const line = rawLine.trim();
        if (!line) {
            flushParagraph();
            flushList();
            continue;
        }

        const headingMatch = line.match(/^(#{2,6})\s+(.+)$/);
        if (headingMatch) {
            flushParagraph();
            flushList();
            blocks.push({
                type: "heading",
                level: headingMatch[1].length,
                text: stripInlineMarkdown(headingMatch[2]),
            });
            continue;
        }

        const unorderedItemMatch = line.match(/^[-*]\s+(.+)$/);
        const orderedItemMatch = line.match(/^\d+[.)]\s+(.+)$/);
        if (unorderedItemMatch || orderedItemMatch) {
            flushParagraph();
            const ordered = Boolean(orderedItemMatch);
            if (!listBlock || listBlock.ordered !== ordered) {
                flushList();
                listBlock = {
                    type: "list",
                    ordered,
                    items: [],
                };
            }
            listBlock.items.push(
                stripInlineMarkdown((orderedItemMatch || unorderedItemMatch)?.[1] ?? ""),
            );
            continue;
        }

        flushList();
        paragraphLines.push(line);
    }

    flushParagraph();
    flushList();

    return {
        title,
        blocks,
    };
};
