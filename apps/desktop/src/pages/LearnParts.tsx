import { useState } from "react";
import { Check, Copy, Search, X } from "lucide-react";
import { useI18n } from "../i18n";
import type { QuizQuestion, Shell } from "../learn/lessons";
import { GLOSSARY, REFERENCE, REF_GROUPS } from "../learn/reference";

export const SHELLS: Shell[] = ["powershell", "cmd", "bash"];
const SHELL_STORAGE_KEY = "autodock.learn.shell";

/** PowerShell on Windows, bash elsewhere, unless the reader already chose. */
export function loadShell(): Shell {
  try {
    const saved = window.localStorage.getItem(SHELL_STORAGE_KEY);
    if (saved === "bash" || saved === "powershell" || saved === "cmd") return saved;
  } catch {
    // Storage can be unavailable: fall back to the detected system.
  }
  const system = typeof navigator !== "undefined" ? navigator.userAgent : "";
  return /windows/i.test(system) ? "powershell" : "bash";
}

export function saveShell(shell: Shell) {
  try {
    window.localStorage.setItem(SHELL_STORAGE_KEY, shell);
  } catch {
    // The choice then lasts only for this session.
  }
}

type Variants = { bash: string; powershell?: string; cmd?: string };

function resolve(variants: Variants, shell: Shell) {
  return variants[shell] ?? variants.bash;
}

function hasVariants(variants: Variants) {
  return [variants.powershell, variants.cmd].some(
    (value) => value !== undefined && value !== variants.bash,
  );
}

export function ShellTabs({ shell, onChange }: { shell: Shell; onChange: (shell: Shell) => void }) {
  const { t } = useI18n();
  return (
    <div className="learn-shell-tabs" role="tablist" aria-label={t("learn.shell.aria")}>
      {SHELLS.map((item) => (
        <button
          key={item}
          type="button"
          role="tab"
          aria-selected={shell === item}
          className={shell === item ? "is-active" : ""}
          onClick={() => onChange(item)}
        >
          {t(`learn.shell.${item}`)}
        </button>
      ))}
    </div>
  );
}

export function CopyButton({
  text,
  copied,
  onCopy,
}: {
  text: string;
  copied: boolean;
  onCopy: (text: string) => void;
}) {
  const { t } = useI18n();
  return (
    <button type="button" className="link-button" onClick={() => onCopy(text)}>
      {copied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
      {copied ? t("learn.copied") : t("learn.copyCommand")}
    </button>
  );
}

/** A command with one spelling per terminal; tabs appear only when they differ. */
export function ShellCode({
  caption,
  variants,
  shell,
  onShellChange,
  copiedKey,
  copyKey,
  onCopy,
}: {
  caption?: string;
  variants: Variants;
  shell: Shell;
  onShellChange: (shell: Shell) => void;
  copiedKey: string;
  copyKey: string;
  onCopy: (key: string, text: string) => void;
}) {
  const text = resolve(variants, shell);
  return (
    <figure className="learn-code">
      {(caption || hasVariants(variants)) && (
        <figcaption>
          {caption && <span>{caption}</span>}
          {hasVariants(variants) && <ShellTabs shell={shell} onChange={onShellChange} />}
        </figcaption>
      )}
      <pre>
        <code>{text}</code>
      </pre>
      <CopyButton text={text} copied={copiedKey === copyKey} onCopy={(value) => onCopy(copyKey, value)} />
    </figure>
  );
}

/** Self-check: picking an option shows right away whether it is correct and why. */
export function Quiz({ questions }: { questions: QuizQuestion[] }) {
  const { t, lang } = useI18n();
  const [answers, setAnswers] = useState<Record<number, number>>({});
  const answered = Object.keys(answers).length;
  const correct = questions.filter((question, index) => answers[index] === question.answer).length;

  return (
    <section className="learn-quiz" aria-labelledby="learn-quiz-title">
      <h3 id="learn-quiz-title" className="learn-subtitle">
        {t("learn.quiz.title")}
      </h3>
      {questions.map((question, qIndex) => {
        const picked = answers[qIndex];
        const isRight = picked === question.answer;
        return (
          <fieldset key={qIndex} className="learn-question">
            <legend>{question.question[lang]}</legend>
            {question.options.map((option, oIndex) => {
              const state =
                picked === undefined
                  ? ""
                  : oIndex === question.answer
                    ? "is-right"
                    : oIndex === picked
                      ? "is-wrong"
                      : "";
              return (
                <label key={oIndex} className={`learn-option ${state}`}>
                  <input
                    type="radio"
                    name={`quiz-${qIndex}`}
                    checked={picked === oIndex}
                    onChange={() => setAnswers((current) => ({ ...current, [qIndex]: oIndex }))}
                  />
                  <span>{option[lang]}</span>
                </label>
              );
            })}
            <p className={`learn-feedback${picked === undefined ? "" : isRight ? " is-right" : " is-wrong"}`} aria-live="polite">
              {picked === undefined ? (
                t("learn.quiz.hint")
              ) : (
                <>
                  {isRight ? <Check size={14} aria-hidden="true" /> : <X size={14} aria-hidden="true" />}{" "}
                  <strong>{isRight ? t("learn.quiz.correct") : t("learn.quiz.incorrect")}</strong>{" "}
                  {question.why[lang]}
                </>
              )}
            </p>
          </fieldset>
        );
      })}
      {answered === questions.length && (
        <p className="learn-score">{t("learn.quiz.score", { n: correct, total: questions.length })}</p>
      )}
    </section>
  );
}

function SearchBox({
  value,
  onChange,
  label,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  label: string;
  placeholder: string;
}) {
  return (
    <label className="learn-search">
      <Search size={16} aria-hidden="true" />
      <input
        type="search"
        value={value}
        placeholder={placeholder}
        aria-label={label}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

export function matches(query: string, ...texts: string[]) {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return texts.some((text) => text.toLowerCase().includes(needle));
}

export { SearchBox };

export function ReferenceTab({
  shell,
  onShellChange,
  copiedKey,
  onCopy,
}: {
  shell: Shell;
  onShellChange: (shell: Shell) => void;
  copiedKey: string;
  onCopy: (key: string, text: string) => void;
}) {
  const { t, lang } = useI18n();
  const [query, setQuery] = useState("");
  const groups = REF_GROUPS.map((group) => ({
    group,
    items: REFERENCE.filter(
      (item) =>
        item.group === group.id &&
        matches(query, item.bash, item.powershell ?? "", item.cmd ?? "", item.description[lang]),
    ),
  })).filter((entry) => entry.items.length > 0);

  return (
    <div className="learn-reference">
      <p className="section-description">{t("learn.ref.description")}</p>
      <div className="learn-toolbar">
        <SearchBox
          value={query}
          onChange={setQuery}
          label={t("learn.search.reference")}
          placeholder={t("learn.search.reference")}
        />
        <ShellTabs shell={shell} onChange={onShellChange} />
      </div>
      {groups.length === 0 && <p className="learn-empty">{t("learn.ref.empty")}</p>}
      {groups.map(({ group, items }) => (
        <section key={group.id} className="learn-ref-group" aria-label={group.title[lang]}>
          <h3>{group.title[lang]}</h3>
          <ul>
            {items.map((item) => {
              const text = resolve(item, shell);
              return (
                <li key={item.id}>
                  <div>
                    <code>{text}</code>
                    <span>{item.description[lang]}</span>
                  </div>
                  <CopyButton
                    text={text}
                    copied={copiedKey === `ref-${item.id}`}
                    onCopy={(value) => onCopy(`ref-${item.id}`, value)}
                  />
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}

export function GlossaryTab() {
  const { t, lang } = useI18n();
  const [query, setQuery] = useState("");
  const terms = GLOSSARY.filter((entry) =>
    matches(query, entry.term.en, entry.term.pt, entry.definition[lang]),
  ).sort((a, b) => a.term[lang].localeCompare(b.term[lang], lang));

  return (
    <div className="learn-glossary">
      <p className="section-description">{t("learn.glossary.description")}</p>
      <div className="learn-toolbar">
        <SearchBox
          value={query}
          onChange={setQuery}
          label={t("learn.search.glossary")}
          placeholder={t("learn.search.glossary")}
        />
      </div>
      {terms.length === 0 && <p className="learn-empty">{t("learn.glossary.empty")}</p>}
      <dl>
        {terms.map((entry) => (
          <div key={entry.id}>
            <dt>{entry.term[lang]}</dt>
            <dd>{entry.definition[lang]}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
