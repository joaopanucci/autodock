import { useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle2,
  Clock,
  Lightbulb,
  TriangleAlert,
} from "lucide-react";
import { Badge, Panel } from "../components/ui";
import { useI18n } from "../i18n";
import { LESSONS, TRACKS, type Block, type Lesson, type Shell } from "../learn/lessons";
import type { Page } from "../types";
import {
  CopyButton,
  GlossaryTab,
  Quiz,
  ReferenceTab,
  SearchBox,
  ShellCode,
  loadShell,
  matches,
  saveShell,
} from "./LearnParts";

const DONE_KEY = "autodock.learn.done";
const LAST_KEY = "autodock.learn.last";

function loadDone(): string[] {
  try {
    const raw = window.localStorage.getItem(DONE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((id): id is string => typeof id === "string")
      : [];
  } catch {
    return [];
  }
}

function saveDone(ids: string[]) {
  try {
    window.localStorage.setItem(DONE_KEY, JSON.stringify(ids));
  } catch {
    // Storage can be unavailable; progress then lasts only for this session.
  }
}

function loadLast(): string | null {
  try {
    const id = window.localStorage.getItem(LAST_KEY);
    return LESSONS.some((lesson) => lesson.id === id) ? id : null;
  } catch {
    return null;
  }
}

function saveLast(id: string) {
  try {
    window.localStorage.setItem(LAST_KEY, id);
  } catch {
    // Not remembering the last lesson is harmless.
  }
}

type TabId = "lessons" | "reference" | "glossary";
type Filter = "all" | "beginner" | "intermediate" | "todo";
const FILTERS: Filter[] = ["all", "beginner", "intermediate", "todo"];

export function Learn({ navigate }: { navigate: (page: Page) => void }) {
  const { t, lang } = useI18n();
  const [done, setDone] = useState<string[]>(loadDone);
  const [last, setLast] = useState<string | null>(loadLast);
  const [openId, setOpenId] = useState<string | null>(null);
  const [tab, setTab] = useState<TabId>("lessons");
  const [shell, setShellState] = useState<Shell>(loadShell);
  const [copied, setCopied] = useState("");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");

  useEffect(() => saveDone(done), [done]);

  const setShell = (next: Shell) => {
    setShellState(next);
    saveShell(next);
  };

  const openLesson = (id: string) => {
    setOpenId(id);
    setLast(id);
    saveLast(id);
    window.scrollTo?.({ top: 0 });
  };

  const lessonIndex = LESSONS.findIndex((item) => item.id === openId);
  const lesson = lessonIndex >= 0 ? LESSONS[lessonIndex] : null;

  const toggle = (id: string) =>
    setDone((current) =>
      current.includes(id) ? current.filter((item) => item !== id) : [...current, id],
    );

  async function copy(key: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(key);
      window.setTimeout(() => setCopied((now) => (now === key ? "" : now)), 1800);
    } catch {
      // Clipboard can be blocked; the text stays visible to select by hand.
    }
  }

  const levelLabel = (item: Lesson) => t(`learn.level.${item.level}`);

  function renderBlock(block: Block, index: number) {
    switch (block.kind) {
      case "p":
        return <p key={index}>{block.text[lang]}</p>;
      case "tip":
      case "warn": {
        const Icon = block.kind === "tip" ? Lightbulb : TriangleAlert;
        return (
          <div key={index} className={`learn-callout learn-${block.kind}`}>
            <Icon size={18} aria-hidden="true" />
            <p>{block.text[lang]}</p>
          </div>
        );
      }
      case "steps":
        return (
          <ol key={index} className="learn-steps">
            {block.items.map((item, i) => (
              <li key={i}>{item[lang]}</li>
            ))}
          </ol>
        );
      case "list":
        return (
          <ul key={index} className="learn-list">
            {block.items.map((item, i) => (
              <li key={i}>{item[lang]}</li>
            ))}
          </ul>
        );
      case "code":
        return (
          <figure key={index} className="learn-code">
            {block.caption && <figcaption>{block.caption[lang]}</figcaption>}
            <pre>
              <code>{block.code}</code>
            </pre>
            <CopyButton
              text={block.code}
              copied={copied === `code-${index}`}
              onCopy={(value) => void copy(`code-${index}`, value)}
            />
          </figure>
        );
      case "shell":
        return (
          <ShellCode
            key={index}
            caption={block.caption?.[lang]}
            variants={block}
            shell={shell}
            onShellChange={setShell}
            copiedKey={copied}
            copyKey={`shell-${index}`}
            onCopy={(key, text) => void copy(key, text)}
          />
        );
    }
  }

  const completed = LESSONS.filter((item) => done.includes(item.id)).length;

  // ───────────── A single lesson ─────────────
  if (lesson) {
    const finished = done.includes(lesson.id);
    const previous = lessonIndex > 0 ? LESSONS[lessonIndex - 1] : null;
    const next = lessonIndex < LESSONS.length - 1 ? LESSONS[lessonIndex + 1] : null;
    const track = TRACKS.find((item) => item.id === lesson.track);
    return (
      <div className="learn">
        <button type="button" className="link-button" onClick={() => setOpenId(null)}>
          <ArrowLeft size={16} aria-hidden="true" /> {t("learn.back")}
        </button>
        <Panel
          title={lesson.title[lang]}
          eyebrow={`${t("learn.lessonOf", { n: lessonIndex + 1, total: LESSONS.length })}${track ? ` · ${track.title[lang]}` : ""}`}
          action={
            <span className="learn-header-meta">
              <Badge tone={finished ? "green" : "blue"}>{levelLabel(lesson)}</Badge>
              <span className="learn-minutes">
                <Clock size={13} aria-hidden="true" /> {t("learn.minutes", { n: lesson.minutes })}
              </span>
            </span>
          }
        >
          <p className="section-description">{lesson.summary[lang]}</p>
          <div className="learn-body">{lesson.blocks.map(renderBlock)}</div>

          <Quiz key={lesson.id} questions={lesson.quiz} />

          {lesson.tryIn && (
            <button
              type="button"
              className="button primary"
              onClick={() => navigate(lesson.tryIn!.page)}
            >
              {lesson.tryIn.label[lang]}
            </button>
          )}

          <h3 className="learn-subtitle">{t("learn.docs")}</h3>
          <ul className="learn-links">
            {lesson.docs.map((doc) => (
              <li key={doc.url}>
                <strong>{doc.label[lang]}</strong>
                <code>{doc.url}</code>
                <button
                  type="button"
                  className="link-button"
                  onClick={() => void copy(doc.url, doc.url)}
                >
                  {copied === doc.url ? t("learn.copied") : t("learn.copyLink")}
                </button>
              </li>
            ))}
          </ul>

          <button type="button" className="button" onClick={() => toggle(lesson.id)}>
            <CheckCircle2 size={16} aria-hidden="true" />
            {finished ? t("learn.markUndone") : t("learn.markDone")}
          </button>
        </Panel>

        <nav className="learn-pager" aria-label={t("learn.pager.aria")}>
          {previous ? (
            <button type="button" className="button" onClick={() => openLesson(previous.id)}>
              <ArrowLeft size={16} aria-hidden="true" />
              <span>
                <small>{t("learn.prev")}</small>
                {previous.title[lang]}
              </span>
            </button>
          ) : (
            <span />
          )}
          {next && (
            <button type="button" className="button" onClick={() => openLesson(next.id)}>
              <span>
                <small>{t("learn.next")}</small>
                {next.title[lang]}
              </span>
              <ArrowRight size={16} aria-hidden="true" />
            </button>
          )}
        </nav>
      </div>
    );
  }

  // ───────────── Lesson list ─────────────
  const resume =
    LESSONS.find((item) => item.id === last && !done.includes(item.id)) ??
    LESSONS.find((item) => !done.includes(item.id)) ??
    null;

  const visible = (item: Lesson) => {
    if (filter === "beginner" || filter === "intermediate") {
      if (item.level !== filter) return false;
    }
    if (filter === "todo" && done.includes(item.id)) return false;
    return matches(query, item.title.en, item.title.pt, item.summary[lang]);
  };
  const shown = LESSONS.filter(visible);

  const tabs: { id: TabId; label: string }[] = [
    { id: "lessons", label: t("learn.tab.lessons") },
    { id: "reference", label: t("learn.tab.reference") },
    { id: "glossary", label: t("learn.tab.glossary") },
  ];

  return (
    <div className="learn">
      <Panel title={t("page.learn.title")} eyebrow={t("learn.eyebrow")}>
        <p className="section-description">{t("page.learn.description")}</p>
        <div
          className="learn-progress"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={LESSONS.length}
          aria-valuenow={completed}
          aria-label={t("learn.progress", { done: completed, total: LESSONS.length })}
        >
          <span style={{ width: `${(completed / LESSONS.length) * 100}%` }} />
        </div>
        <small>
          {t("learn.progress", { done: completed, total: LESSONS.length })} · {t("learn.private")}
        </small>
        <div className="learn-resume">
          {resume ? (
            <button type="button" className="button primary" onClick={() => openLesson(resume.id)}>
              {t("learn.continue", { title: resume.title[lang] })}
              <ArrowRight size={16} aria-hidden="true" />
            </button>
          ) : (
            <p>{t("learn.allDone")}</p>
          )}
        </div>
      </Panel>

      <div className="learn-tabs" role="tablist" aria-label={t("learn.tab.aria")}>
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            id={`learn-tab-${item.id}`}
            aria-selected={tab === item.id}
            aria-controls={`learn-panel-${item.id}`}
            className={tab === item.id ? "is-active" : ""}
            onClick={() => setTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>

      {tab === "lessons" && (
        <div role="tabpanel" id="learn-panel-lessons" aria-labelledby="learn-tab-lessons" className="learn">
          <div className="learn-toolbar">
            <SearchBox
              value={query}
              onChange={setQuery}
              label={t("learn.search.lessons")}
              placeholder={t("learn.search.placeholder")}
            />
            <div className="learn-filters" role="group" aria-label={t("learn.filter.aria")}>
              {FILTERS.map((item) => (
                <button
                  key={item}
                  type="button"
                  aria-pressed={filter === item}
                  className={filter === item ? "is-active" : ""}
                  onClick={() => setFilter(item)}
                >
                  {item === "all" || item === "todo" ? t(`learn.filter.${item}`) : t(`learn.level.${item}`)}
                </button>
              ))}
            </div>
          </div>

          {shown.length === 0 && (
            <div className="learn-empty">
              <h3>{t("learn.empty.title")}</h3>
              <p>{t("learn.empty.body")}</p>
            </div>
          )}

          {TRACKS.map((track) => {
            const inTrack = shown.filter((item) => item.track === track.id);
            if (inTrack.length === 0) return null;
            const all = LESSONS.filter((item) => item.track === track.id);
            const finishedInTrack = all.filter((item) => done.includes(item.id)).length;
            return (
              <section key={track.id} className="learn-track" aria-label={track.title[lang]}>
                <header>
                  <div>
                    <h3>{track.title[lang]}</h3>
                    <p>{track.description[lang]}</p>
                  </div>
                  <span className="learn-track-count">
                    {t("learn.track.count", { done: finishedInTrack, total: all.length })}
                  </span>
                </header>
                <div className="learn-grid">
                  {inTrack.map((item) => {
                    const finished = done.includes(item.id);
                    const number = LESSONS.indexOf(item) + 1;
                    return (
                      <article key={item.id} className={`learn-card${finished ? " is-done" : ""}`}>
                        <div className="learn-card-top">
                          <span className="learn-number">
                            {finished ? <Check size={14} aria-hidden="true" /> : number}
                          </span>
                          <Badge tone={item.level === "beginner" ? "green" : "amber"}>
                            {levelLabel(item)}
                          </Badge>
                          <span className="learn-minutes">
                            <Clock size={13} aria-hidden="true" /> {t("learn.minutes", { n: item.minutes })}
                          </span>
                        </div>
                        <h3>{item.title[lang]}</h3>
                        <p>{item.summary[lang]}</p>
                        <button type="button" className="button" onClick={() => openLesson(item.id)}>
                          {finished ? t("learn.review") : t("learn.start")}
                          <span className="sr-only"> — {item.title[lang]}</span>
                        </button>
                      </article>
                    );
                  })}
                </div>
              </section>
            );
          })}
          <p className="learn-note">{t("learn.nextSteps")}</p>
        </div>
      )}

      {tab === "reference" && (
        <div role="tabpanel" id="learn-panel-reference" aria-labelledby="learn-tab-reference">
          <Panel title={t("learn.ref.title")} eyebrow={t("learn.eyebrow")}>
            <ReferenceTab
              shell={shell}
              onShellChange={setShell}
              copiedKey={copied}
              onCopy={(key, text) => void copy(key, text)}
            />
          </Panel>
        </div>
      )}

      {tab === "glossary" && (
        <div role="tabpanel" id="learn-panel-glossary" aria-labelledby="learn-tab-glossary">
          <Panel title={t("learn.glossary.title")} eyebrow={t("learn.eyebrow")}>
            <GlossaryTab />
          </Panel>
        </div>
      )}
    </div>
  );
}
