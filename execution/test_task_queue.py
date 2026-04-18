from __future__ import annotations

import argparse
import datetime as dt
import io
import unittest
import uuid
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

import task_queue


def make_repo_root() -> Path:
    temp_root = Path(__file__).resolve().parent.parent / ".tmp" / "execution-tests"
    temp_root.mkdir(parents=True, exist_ok=True)
    root = temp_root / f"tasks-{uuid.uuid4().hex}"
    root.mkdir()
    return root


def write_task_file(
    root: Path,
    name: str,
    fields: dict[str, str],
    body: str = "",
) -> Path:
    path = root / name
    lines = ["---"]
    for key, value in fields.items():
        lines.append(f"{key}: {value}")
    lines.append("---")
    text = "\n".join(lines) + "\n" + body
    path.write_text(text, encoding="utf-8")
    return path


class TaskPatternTests(unittest.TestCase):
    def test_matches_canonical_task_filename(self) -> None:
        self.assertIsNotNone(task_queue.TASK_PATTERN.match("001-T-name.md"))
        self.assertIsNotNone(task_queue.TASK_PATTERN.match("42-T-x.md"))

    def test_rejects_missing_T_marker(self) -> None:
        self.assertIsNone(task_queue.TASK_PATTERN.match("001-X-name.md"))

    def test_rejects_non_md_extension(self) -> None:
        self.assertIsNone(task_queue.TASK_PATTERN.match("001-T-name.txt"))

    def test_rejects_filename_without_numeric_prefix(self) -> None:
        self.assertIsNone(task_queue.TASK_PATTERN.match("T-name.md"))


class TaskClassTests(unittest.TestCase):
    def test_task_id_prefers_fields_value_when_numeric(self) -> None:
        task = task_queue.Task(
            path=Path("009-T-something.md"),
            fields={"task_id": "42"},
            body="",
        )
        self.assertEqual(task.task_id, 42)

    def test_task_id_strips_whitespace_in_field_value(self) -> None:
        task = task_queue.Task(
            path=Path("009-T-x.md"),
            fields={"task_id": "  42  "},
            body="",
        )
        self.assertEqual(task.task_id, 42)

    def test_task_id_falls_back_to_filename_prefix(self) -> None:
        task = task_queue.Task(
            path=Path("007-T-something.md"),
            fields={},
            body="",
        )
        self.assertEqual(task.task_id, 7)

    def test_task_id_falls_back_when_field_is_non_numeric(self) -> None:
        task = task_queue.Task(
            path=Path("007-T-something.md"),
            fields={"task_id": "abc"},
            body="",
        )
        self.assertEqual(task.task_id, 7)

    def test_status_returns_field_value(self) -> None:
        task = task_queue.Task(
            path=Path("1-T-x.md"), fields={"status": "pending"}, body=""
        )
        self.assertEqual(task.status, "pending")

    def test_status_defaults_to_empty_string(self) -> None:
        task = task_queue.Task(path=Path("1-T-x.md"), fields={}, body="")
        self.assertEqual(task.status, "")

    def test_title_returns_field_value(self) -> None:
        task = task_queue.Task(
            path=Path("1-T-slug.md"),
            fields={"title": "Custom Title"},
            body="",
        )
        self.assertEqual(task.title, "Custom Title")

    def test_title_defaults_to_path_stem(self) -> None:
        task = task_queue.Task(path=Path("1-T-slug.md"), fields={}, body="")
        self.assertEqual(task.title, "1-T-slug")


class ParseFrontMatterTests(unittest.TestCase):
    def test_parses_fields_and_body(self) -> None:
        text = "---\ntitle: Hello\nstatus: pending\n---\nbody content\n"
        fields, body = task_queue.parse_front_matter(text)
        self.assertEqual(fields, {"title": "Hello", "status": "pending"})
        self.assertEqual(body, "body content\n")

    def test_splits_on_first_colon_only(self) -> None:
        text = "---\nurl: https://example.com/path\n---\n"
        fields, _ = task_queue.parse_front_matter(text)
        self.assertEqual(fields, {"url": "https://example.com/path"})

    def test_strips_whitespace_from_keys_and_values(self) -> None:
        text = "---\n  key  :   value  \n---\n"
        fields, _ = task_queue.parse_front_matter(text)
        self.assertEqual(fields, {"key": "value"})

    def test_skips_blank_lines_inside_front_matter(self) -> None:
        text = "---\ntitle: A\n\nstatus: pending\n---\n"
        fields, _ = task_queue.parse_front_matter(text)
        self.assertEqual(fields, {"title": "A", "status": "pending"})

    def test_empty_front_matter_returns_empty_fields(self) -> None:
        text = "---\n\n---\nbody\n"
        fields, body = task_queue.parse_front_matter(text)
        self.assertEqual(fields, {})
        self.assertEqual(body, "body\n")

    def test_raises_when_front_matter_missing(self) -> None:
        with self.assertRaises(ValueError) as ctx:
            task_queue.parse_front_matter("no front matter here\n")
        self.assertIn("missing front matter", str(ctx.exception))

    def test_raises_when_line_has_no_colon(self) -> None:
        text = "---\nthis line has no colon\n---\n"
        with self.assertRaises(ValueError) as ctx:
            task_queue.parse_front_matter(text)
        self.assertIn("invalid front matter line", str(ctx.exception))


class SerializeTaskTests(unittest.TestCase):
    def test_orders_canonical_keys_before_extras(self) -> None:
        task = task_queue.Task(
            path=Path("001-T-x.md"),
            fields={
                "zzz_extra": "z",
                "doc_type": "task",
                "status": "pending",
                "title": "Example",
                "task_id": "1",
            },
            body="Body.\n",
        )
        rendered = task_queue.serialize_task(task)
        keys_in_order = [
            line.split(":", 1)[0]
            for line in rendered.splitlines()
            if ":" in line and not line.startswith("---")
        ]
        doc_type_idx = keys_in_order.index("doc_type")
        task_id_idx = keys_in_order.index("task_id")
        title_idx = keys_in_order.index("title")
        status_idx = keys_in_order.index("status")
        extra_idx = keys_in_order.index("zzz_extra")
        self.assertLess(doc_type_idx, task_id_idx)
        self.assertLess(task_id_idx, title_idx)
        self.assertLess(title_idx, status_idx)
        self.assertLess(status_idx, extra_idx)

    def test_sorts_extra_keys_alphabetically(self) -> None:
        task = task_queue.Task(
            path=Path("001-T-x.md"),
            fields={"banana": "b", "apple": "a"},
            body="",
        )
        rendered = task_queue.serialize_task(task)
        self.assertLess(rendered.index("apple:"), rendered.index("banana:"))

    def test_emits_triple_dash_delimiters_and_preserves_body(self) -> None:
        task = task_queue.Task(
            path=Path("001-T-x.md"),
            fields={"title": "t"},
            body="line one\nline two\n",
        )
        rendered = task_queue.serialize_task(task)
        self.assertTrue(rendered.startswith("---\n"))
        self.assertIn("\n---\n", rendered)
        self.assertTrue(rendered.endswith("line one\nline two\n"))

    def test_round_trip_preserves_fields_and_body(self) -> None:
        original = task_queue.Task(
            path=Path("001-T-example.md"),
            fields={
                "doc_type": "task",
                "task_id": "1",
                "title": "Example",
                "status": "pending",
                "custom": "value",
            },
            body="Body text.\n",
        )
        rendered = task_queue.serialize_task(original)
        fields, body = task_queue.parse_front_matter(rendered)
        self.assertEqual(fields, original.fields)
        self.assertEqual(body, original.body)


class LoadTasksTests(unittest.TestCase):
    def test_loads_matching_files_and_parses_front_matter(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-alpha.md",
            {"task_id": "1", "status": "pending", "title": "Alpha"},
        )
        write_task_file(
            root,
            "002-T-beta.md",
            {"task_id": "2", "status": "done", "title": "Beta"},
        )
        tasks = task_queue.load_tasks(root)
        self.assertEqual([t.path.name for t in tasks], ["001-T-alpha.md", "002-T-beta.md"])
        self.assertEqual(tasks[0].title, "Alpha")
        self.assertEqual(tasks[1].status, "done")

    def test_skips_non_task_files(self) -> None:
        root = make_repo_root()
        write_task_file(root, "001-T-alpha.md", {"task_id": "1", "status": "pending"})
        (root / "README.md").write_text("not a task\n", encoding="utf-8")
        (root / "001-X-junk.md").write_text("irrelevant\n", encoding="utf-8")
        tasks = task_queue.load_tasks(root)
        self.assertEqual([t.path.name for t in tasks], ["001-T-alpha.md"])

    def test_sorts_by_task_id_numerically(self) -> None:
        root = make_repo_root()
        write_task_file(root, "010-T-later.md", {"task_id": "10", "status": "pending"})
        write_task_file(root, "002-T-early.md", {"task_id": "2", "status": "pending"})
        tasks = task_queue.load_tasks(root)
        self.assertEqual([t.task_id for t in tasks], [2, 10])

    def test_recurses_into_subdirectories(self) -> None:
        root = make_repo_root()
        sub = root / "phase-1"
        sub.mkdir()
        write_task_file(sub, "001-T-nested.md", {"task_id": "1", "status": "pending"})
        tasks = task_queue.load_tasks(root)
        self.assertEqual([t.path.name for t in tasks], ["001-T-nested.md"])

    def test_empty_directory_yields_no_tasks(self) -> None:
        root = make_repo_root()
        self.assertEqual(task_queue.load_tasks(root), [])


class SaveTaskTests(unittest.TestCase):
    def test_save_then_load_round_trips_task(self) -> None:
        root = make_repo_root()
        task = task_queue.Task(
            path=root / "001-T-example.md",
            fields={"task_id": "1", "status": "pending", "title": "Example"},
            body="Body content.\n",
        )
        task_queue.save_task(task)
        loaded = task_queue.load_tasks(root)
        self.assertEqual(len(loaded), 1)
        self.assertEqual(loaded[0].fields, task.fields)
        self.assertEqual(loaded[0].body, task.body)


class NowDateTests(unittest.TestCase):
    def test_returns_today_in_iso_format(self) -> None:
        self.assertEqual(task_queue.now_date(), dt.date.today().isoformat())


class EnsureSingleInProgressTests(unittest.TestCase):
    def _make(self, name: str, status: str) -> task_queue.Task:
        return task_queue.Task(path=Path(name), fields={"status": status}, body="")

    def test_returns_none_when_no_in_progress_tasks(self) -> None:
        tasks = [self._make("001-T-a.md", "pending"), self._make("002-T-b.md", "done")]
        self.assertIsNone(task_queue.ensure_single_in_progress(tasks))

    def test_returns_the_single_in_progress_task(self) -> None:
        current = self._make("001-T-a.md", "in_progress")
        tasks = [self._make("002-T-b.md", "pending"), current]
        self.assertIs(task_queue.ensure_single_in_progress(tasks), current)

    def test_raises_system_exit_when_multiple_in_progress(self) -> None:
        tasks = [
            self._make("001-T-a.md", "in_progress"),
            self._make("002-T-b.md", "in_progress"),
        ]
        with self.assertRaises(SystemExit) as ctx:
            task_queue.ensure_single_in_progress(tasks)
        message = str(ctx.exception)
        self.assertIn("001-T-a.md", message)
        self.assertIn("002-T-b.md", message)


class ResolveTaskTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = make_repo_root()
        self.path = self.root / "001-T-alpha.md"
        write_task_file(
            self.root,
            "001-T-alpha.md",
            {"task_id": "1", "status": "pending"},
        )
        self.tasks = task_queue.load_tasks(self.root)

    def test_resolves_by_task_id_string(self) -> None:
        resolved = task_queue.resolve_task(self.tasks, "1")
        self.assertEqual(resolved.path.name, "001-T-alpha.md")

    def test_resolves_by_path_filename(self) -> None:
        resolved = task_queue.resolve_task(self.tasks, "001-T-alpha.md")
        self.assertEqual(resolved.path.name, "001-T-alpha.md")

    def test_resolves_by_path_stem(self) -> None:
        resolved = task_queue.resolve_task(self.tasks, "001-T-alpha")
        self.assertEqual(resolved.path.name, "001-T-alpha.md")

    def test_resolves_by_full_path_string(self) -> None:
        resolved = task_queue.resolve_task(self.tasks, str(self.path))
        self.assertEqual(resolved.path.name, "001-T-alpha.md")

    def test_raises_system_exit_when_no_match(self) -> None:
        with self.assertRaises(SystemExit) as ctx:
            task_queue.resolve_task(self.tasks, "missing")
        self.assertIn("task not found", str(ctx.exception))


class NextPendingTests(unittest.TestCase):
    def _make(self, name: str, status: str, next_task: str = "") -> task_queue.Task:
        return task_queue.Task(
            path=Path(name),
            fields={"status": status, "next_task": next_task},
            body="",
        )

    def test_follows_explicit_next_task_pointer_when_pending(self) -> None:
        t1 = self._make("001-T-a.md", "done", next_task="003-T-c.md")
        t2 = self._make("002-T-b.md", "pending")
        t3 = self._make("003-T-c.md", "pending")
        result = task_queue.next_pending([t1, t2, t3], current=t1)
        self.assertIs(result, t3)

    def test_falls_back_when_pointed_next_task_is_not_pending(self) -> None:
        t1 = self._make("001-T-a.md", "done", next_task="002-T-b.md")
        t2 = self._make("002-T-b.md", "done")
        t3 = self._make("003-T-c.md", "pending")
        result = task_queue.next_pending([t1, t2, t3], current=t1)
        self.assertIs(result, t3)

    def test_falls_back_when_pointed_next_task_is_missing(self) -> None:
        t1 = self._make("001-T-a.md", "done", next_task="999-T-gone.md")
        t2 = self._make("002-T-b.md", "pending")
        result = task_queue.next_pending([t1, t2], current=t1)
        self.assertIs(result, t2)

    def test_returns_first_pending_when_no_current_task(self) -> None:
        t1 = self._make("001-T-a.md", "done")
        t2 = self._make("002-T-b.md", "pending")
        t3 = self._make("003-T-c.md", "pending")
        result = task_queue.next_pending([t1, t2, t3])
        self.assertIs(result, t2)

    def test_returns_none_when_no_pending_exists(self) -> None:
        t1 = self._make("001-T-a.md", "done")
        t2 = self._make("002-T-b.md", "in_progress")
        self.assertIsNone(task_queue.next_pending([t1, t2]))


class PrintTaskTests(unittest.TestCase):
    def test_prints_all_expected_lines(self) -> None:
        task = task_queue.Task(
            path=Path("001-T-x.md"),
            fields={
                "task_id": "1",
                "title": "Sample",
                "status": "pending",
                "parent_plan": "plan.md",
                "prev_task": "000-T-prev.md",
                "next_task": "002-T-next.md",
            },
            body="",
        )
        buf = io.StringIO()
        with redirect_stdout(buf):
            task_queue.print_task(task)
        output = buf.getvalue()
        self.assertIn("path=001-T-x.md", output)
        self.assertIn("task_id=1", output)
        self.assertIn("title=Sample", output)
        self.assertIn("status=pending", output)
        self.assertIn("parent_plan=plan.md", output)
        self.assertIn("prev_task=000-T-prev.md", output)
        self.assertIn("next_task=002-T-next.md", output)


class CmdListTests(unittest.TestCase):
    def test_lists_task_id_status_and_title(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "pending", "title": "Alpha"},
        )
        write_task_file(
            root,
            "002-T-b.md",
            {"task_id": "2", "status": "done", "title": "Beta"},
        )
        args = argparse.Namespace(root=str(root))
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_list(args)
        self.assertEqual(exit_code, 0)
        output = buf.getvalue()
        self.assertIn("001", output)
        self.assertIn("pending", output)
        self.assertIn("Alpha", output)
        self.assertIn("002", output)
        self.assertIn("done", output)
        self.assertIn("Beta", output)


class CmdResumeTests(unittest.TestCase):
    def test_promotes_first_pending_when_none_in_progress(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "pending", "title": "A"},
        )
        args = argparse.Namespace(root=str(root))
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_resume(args)
        self.assertEqual(exit_code, 0)
        loaded = task_queue.load_tasks(root)
        self.assertEqual(loaded[0].status, "in_progress")
        self.assertEqual(loaded[0].fields["updated"], task_queue.now_date())
        self.assertIn("001-T-a.md", buf.getvalue())

    def test_reports_existing_in_progress_without_advancing(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "in_progress", "title": "A"},
        )
        write_task_file(
            root,
            "002-T-b.md",
            {"task_id": "2", "status": "pending", "title": "B"},
        )
        args = argparse.Namespace(root=str(root))
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_resume(args)
        self.assertEqual(exit_code, 0)
        loaded = sorted(task_queue.load_tasks(root), key=lambda t: t.task_id)
        self.assertEqual(loaded[0].status, "in_progress")
        self.assertEqual(loaded[1].status, "pending")
        self.assertIn("001-T-a.md", buf.getvalue())

    def test_reports_no_pending_tasks(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "done", "title": "A"},
        )
        args = argparse.Namespace(root=str(root))
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_resume(args)
        self.assertEqual(exit_code, 0)
        self.assertIn("no pending tasks", buf.getvalue())


class CmdCompleteTests(unittest.TestCase):
    def test_marks_in_progress_task_done(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "in_progress", "title": "A"},
        )
        write_task_file(
            root,
            "002-T-b.md",
            {"task_id": "2", "status": "pending", "title": "B"},
        )
        args = argparse.Namespace(root=str(root), task_ref="1", advance=False)
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_complete(args)
        self.assertEqual(exit_code, 0)
        loaded = sorted(task_queue.load_tasks(root), key=lambda t: t.task_id)
        self.assertEqual(loaded[0].status, "done")
        self.assertEqual(loaded[0].fields["updated"], task_queue.now_date())
        self.assertEqual(loaded[1].status, "pending")

    def test_rejects_completing_a_non_in_progress_task(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "pending", "title": "A"},
        )
        args = argparse.Namespace(root=str(root), task_ref="1", advance=False)
        with self.assertRaises(SystemExit) as ctx:
            with redirect_stdout(io.StringIO()):
                task_queue.cmd_complete(args)
        self.assertIn("not in_progress", str(ctx.exception))

    def test_advance_flag_promotes_next_pending(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {
                "task_id": "1",
                "status": "in_progress",
                "title": "A",
                "next_task": "002-T-b.md",
            },
        )
        write_task_file(
            root,
            "002-T-b.md",
            {"task_id": "2", "status": "pending", "title": "B"},
        )
        args = argparse.Namespace(root=str(root), task_ref="1", advance=True)
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_complete(args)
        self.assertEqual(exit_code, 0)
        loaded = sorted(task_queue.load_tasks(root), key=lambda t: t.task_id)
        self.assertEqual(loaded[0].status, "done")
        self.assertEqual(loaded[1].status, "in_progress")
        self.assertIn("advanced_to:", buf.getvalue())

    def test_advance_reports_no_pending_when_none_remain(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "in_progress", "title": "A"},
        )
        args = argparse.Namespace(root=str(root), task_ref="1", advance=True)
        buf = io.StringIO()
        with redirect_stdout(buf):
            exit_code = task_queue.cmd_complete(args)
        self.assertEqual(exit_code, 0)
        self.assertIn("no pending tasks", buf.getvalue())


class CmdSetStatusTests(unittest.TestCase):
    def test_sets_status_to_pending(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "in_progress", "title": "A"},
        )
        args = argparse.Namespace(root=str(root), task_ref="1", status="pending")
        with redirect_stdout(io.StringIO()):
            exit_code = task_queue.cmd_set_status(args)
        self.assertEqual(exit_code, 0)
        loaded = task_queue.load_tasks(root)
        self.assertEqual(loaded[0].status, "pending")
        self.assertEqual(loaded[0].fields["updated"], task_queue.now_date())

    def test_rejects_second_in_progress(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "in_progress", "title": "A"},
        )
        write_task_file(
            root,
            "002-T-b.md",
            {"task_id": "2", "status": "pending", "title": "B"},
        )
        args = argparse.Namespace(root=str(root), task_ref="2", status="in_progress")
        with self.assertRaises(SystemExit) as ctx:
            with redirect_stdout(io.StringIO()):
                task_queue.cmd_set_status(args)
        self.assertIn("already in_progress", str(ctx.exception))

    def test_promotes_pending_to_in_progress_when_no_conflict(self) -> None:
        root = make_repo_root()
        write_task_file(
            root,
            "001-T-a.md",
            {"task_id": "1", "status": "pending", "title": "A"},
        )
        args = argparse.Namespace(root=str(root), task_ref="1", status="in_progress")
        with redirect_stdout(io.StringIO()):
            exit_code = task_queue.cmd_set_status(args)
        self.assertEqual(exit_code, 0)
        loaded = task_queue.load_tasks(root)
        self.assertEqual(loaded[0].status, "in_progress")


class BuildParserTests(unittest.TestCase):
    def test_parser_requires_a_subcommand(self) -> None:
        parser = task_queue.build_parser()
        with self.assertRaises(SystemExit):
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                parser.parse_args([])

    def test_complete_subcommand_advance_defaults_to_false(self) -> None:
        parser = task_queue.build_parser()
        args = parser.parse_args(["complete", "./root", "1"])
        self.assertEqual(args.command, "complete")
        self.assertFalse(args.advance)

    def test_complete_subcommand_accepts_advance_flag(self) -> None:
        parser = task_queue.build_parser()
        args = parser.parse_args(["complete", "./root", "1", "--advance"])
        self.assertTrue(args.advance)

    def test_set_status_rejects_unknown_status_choice(self) -> None:
        parser = task_queue.build_parser()
        with self.assertRaises(SystemExit):
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                parser.parse_args(["set-status", "./root", "1", "bogus"])

    def test_set_status_accepts_known_status_choices(self) -> None:
        parser = task_queue.build_parser()
        for status in ("pending", "in_progress", "done"):
            args = parser.parse_args(["set-status", "./root", "1", status])
            self.assertEqual(args.status, status)


if __name__ == "__main__":
    unittest.main()
