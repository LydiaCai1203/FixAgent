import { useMemo } from 'react';
import DiffMatchPatch from 'diff-match-patch';

export default function CodeDiffViewer({
  oldCode,
  newCode,
  startLine = 1,
}: {
  oldCode: string;
  newCode: string;
  startLine?: number;
}) {
  const dmp = new DiffMatchPatch();

  const diffLines = useMemo(() => {
    const diffs = dmp.diff_main(oldCode, newCode);
    dmp.diff_cleanupSemantic(diffs);

    const result: Array<{
      type: 'equal' | 'insert' | 'delete';
      oldLineNum: number | null;
      newLineNum: number | null;
      text: string;
    }> = [];

    let oldLineNum = startLine;
    let newLineNum = startLine;

    for (const [op, text] of diffs) {
      const lines = text.split('\n');
      const hasTrailingNewline = text.endsWith('\n');
      const lineCount = hasTrailingNewline ? lines.length - 1 : lines.length;

      for (let i = 0; i < lineCount; i++) {
        const lineText = lines[i];
        if (op === 0) {
          result.push({
            type: 'equal',
            oldLineNum,
            newLineNum,
            text: lineText,
          });
          oldLineNum++;
          newLineNum++;
        } else if (op === -1) {
          result.push({
            type: 'delete',
            oldLineNum,
            newLineNum: null,
            text: lineText,
          });
          oldLineNum++;
        } else if (op === 1) {
          result.push({
            type: 'insert',
            oldLineNum: null,
            newLineNum,
            text: lineText,
          });
          newLineNum++;
        }
      }
    }

    return result;
  }, [oldCode, newCode, startLine]);

  return (
    <div className="diff-viewer">
      {diffLines.map((line, idx) => (
        <div key={idx} className={`diff-line diff-${line.type}`}>
          <span className="diff-gutter diff-gutter-old">
            {line.oldLineNum ?? ''}
          </span>
          <span className="diff-gutter diff-gutter-new">
            {line.newLineNum ?? ''}
          </span>
          <span className="diff-marker">
            {line.type === 'equal' ? ' ' : line.type === 'insert' ? '+' : '-'}
          </span>
          <span className="diff-content">{line.text}</span>
        </div>
      ))}
    </div>
  );
}
