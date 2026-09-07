import copy
import json
import unittest

from kg_decision import parse_stream, validate, classify_child_failure


def events():
    return [
        {"event": "init", "conversation_id": "fixture", "init": {"agent": "kg-decision", "tools": ["view_file"]}},
        {"event": "result", "result": {"conversation_id": "fixture", "status": "SUCCESS",
         "response": "untrusted prose", "structured_output": {
             "action": "wander", "parameters": {"mode": "random"}, "rationale": "explore"}}}]


def wire(items):
    return "\n".join(json.dumps(item) for item in items).encode()


class DecisionTests(unittest.TestCase):
    def test_child_failure_categories(self):
        self.assertEqual(classify_child_failure("Print mode: timed out"), "timeout")
        self.assertEqual(classify_child_failure("permission denied"), "permission")
        self.assertEqual(classify_child_failure("not logged in"), "auth")
        self.assertEqual(classify_child_failure("opaque failure"), "child_exit")
    def test_structured_field_not_prose_or_global_inventory(self):
        self.assertEqual(parse_stream(wire(events()))["action"], "wander")

    def test_bad_event_paths_reject(self):
        cases = []
        e = events(); e[-1]["result"].pop("structured_output"); cases.append(e)
        e = events(); e[-1]["result"]["status"] = "ERROR"; cases.append(e)
        e = events(); e[0]["init"]["agent"] = "default"; cases.append(e)
        e = events(); e[-1]["result"]["conversation_id"] = "other"; cases.append(e)
        e = events(); e.append(copy.deepcopy(e[-1])); cases.append(e)
        e = events(); e.insert(1, {"event": "step_update", "step_update": {
            "conversation_id": "fixture", "step_type": "tool", "tool_name": "view_file", "state": "ACTIVE"}}); cases.append(e)
        cases.append(events()[:1])
        for case in cases:
            with self.subTest(case=case), self.assertRaises(ValueError):
                parse_stream(wire(case))

    def test_truncated_and_duplicate_json_reject(self):
        for raw in [b'{"event":', b'{"event":"init","event":"result"}', wire(events())[:-10]]:
            with self.assertRaises(ValueError):
                parse_stream(raw)

    def test_action_specific_validation(self):
        for action, params in [("connect", {"target": "abc", "rel_type": "related_to"}),
                               ("create_entity", {"name": "moth", "entity_type": "concept"}),
                               ("observe", {"name": "note", "content": "context"})]:
            self.assertEqual(validate({"action": action, "parameters": params, "rationale": "reason"})["action"], action)
        for action, params in [("connect", {}), ("wander", {"mode": "shell"}),
                               ("wander", {"mode": "random", "query": "extra"}),
                               ("observe", {"name": "x", "content": ""})]:
            with self.assertRaises(ValueError):
                validate({"action": action, "parameters": params, "rationale": "reason"})


if __name__ == "__main__":
    unittest.main()
