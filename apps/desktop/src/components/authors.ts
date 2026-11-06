// Agrupa la autoría por versículo en tramos seguidos del mismo autor:
// [1 ana, 2 ana, 3 ana, 4 beto] → "ana (v. 1-3)", "beto (v. 4)".
import type { VerseAuthor } from "../api";

export interface AuthorGroup {
  author: string;
  /** "1-3", "4" o "2-3" (rangos tal como van en el texto). */
  label: string;
  /** Fecha más reciente del tramo; 0 si hay cambios sin confirmar. */
  time: number;
}

const first = (v: string) => parseInt(v, 10) || 0;
const last = (v: string) => {
  const m = /-(\d+)/.exec(v);
  return m ? parseInt(m[1], 10) : first(v);
};

export function authorGroups(authors: VerseAuthor[]): AuthorGroup[] {
  const out: AuthorGroup[] = [];
  let from = "";
  let to = "";
  for (const a of authors) {
    const g = out[out.length - 1];
    if (g && g.author === a.author) {
      to = a.verse;
      g.label = first(from) === last(to) ? from : `${first(from)}-${last(to)}`;
      g.time = g.time === 0 || a.time === 0 ? 0 : Math.max(g.time, a.time);
    } else {
      from = a.verse;
      to = a.verse;
      out.push({ author: a.author, label: a.verse, time: a.time });
    }
  }
  return out;
}
