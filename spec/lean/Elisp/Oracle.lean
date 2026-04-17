/-
  Elisp.Oracle — JSON-in/JSON-out differential testing oracle.

  Protocol:
    - Reads a JSON object from stdin:
      { "expr": <s-expression as nested JSON> }
    - Evaluates with the reference evaluator.
    - Writes a JSON result to stdout:
      { "ok": <value as JSON> } or { "error": <string> }

  JSON encoding for Val:
    null         → Val.nil
    true         → Val.t
    { "int": n } → Val.int n
    { "str": s } → Val.str s
    { "sym": s } → Val.sym s
    { "cons": [car, cdr] }  → Val.cons car cdr
    { "lam": "<closure>" }  → Val.lam (not round-tripped)
-/

import Elisp.Ast
import Elisp.Env
import Elisp.Eval
import Lean.Data.Json

open Lean (Json JsonNumber)

namespace Elisp.Oracle

/-- Extract integer from a JsonNumber (ignoring exponent for simplicity). -/
private def jsonNumToInt (n : JsonNumber) : Int := n.mantissa

/-- Look up a key in a Json object. -/
private def objFind? (j : Json) (key : String) : Option Json :=
  match j with
  | .obj m => m[key]?
  | _      => none

/-- Decode a JSON value into an Elisp Val. -/
partial def jsonToVal : Json → Except String Val
  | .null => .ok .nil
  | .bool true => .ok .t
  | .bool false => .ok .nil
  | .num n => .ok (.int (jsonNumToInt n))
  | .str s => .ok (.str s)
  | j@(.obj _) =>
    if let some v := objFind? j "int" then
      match v with
      | .num n => .ok (.int (jsonNumToInt n))
      | _      => .error "int field must be a number"
    else if let some v := objFind? j "str" then
      match v with
      | .str s => .ok (.str s)
      | _      => .error "str field must be a string"
    else if let some v := objFind? j "sym" then
      match v with
      | .str s => .ok (.sym s)
      | _      => .error "sym field must be a string"
    else if let some v := objFind? j "cons" then
      match v with
      | .arr a =>
        if h : a.size = 2 then do
          let car ← jsonToVal a[0]
          let cdr ← jsonToVal a[1]
          .ok (.cons car cdr)
        else .error "cons must have exactly 2 elements"
      | _ => .error "cons field must be an array"
    else .error "unknown object keys"
  | .arr a => do
    let vals ← a.toList.mapM jsonToVal
    .ok (vals.foldr Val.cons .nil)

/-- Encode an Elisp Val as JSON. -/
partial def valToJson : Val → Json
  | .nil       => .null
  | .t         => .bool true
  | .int n     => Json.mkObj [("int", .num ⟨n, 0⟩)]
  | .str s     => Json.mkObj [("str", .str s)]
  | .sym s     => Json.mkObj [("sym", .str s)]
  | .cons a b  => Json.mkObj [("cons", .arr #[valToJson a, valToJson b])]
  | .lam ..    => Json.mkObj [("lam", .str "<closure>")]

/-- Run the oracle on a single JSON input. -/
def runOracle (input : Json) : Json :=
  let exprJson := input.getObjValD "expr"
  match jsonToVal exprJson with
  | .error msg => Json.mkObj [("error", .str s!"parse error: {msg}")]
  | .ok expr =>
    let env : Env := {}
    match eval env expr with
    | .ok (_, val) => Json.mkObj [("ok", valToJson val)]
    | .error (.thrown tag value) =>
      Json.mkObj [("error", .str s!"uncaught throw: {repr tag}"),
                  ("value", valToJson value)]
    | .error e => Json.mkObj [("error", .str s!"{repr e}")]

end Elisp.Oracle

open Elisp.Oracle in
def main : IO Unit := do
  let stdin ← IO.getStdin
  let input ← stdin.getLine
  match Json.parse input with
  | .error msg => IO.println s!"\{\"error\": \"json parse: {msg}\"}"
  | .ok json =>
    let result := runOracle json
    IO.println (toString result)
