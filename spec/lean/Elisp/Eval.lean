/-
  Elisp.Eval — Reference evaluator for the core subset.

  Uses `partial def` to avoid termination obligations. This is fine for
  a differential-testing oracle. To enable proofs later, convert to a
  fuel-indexed or small-step version.
-/

import Elisp.Ast
import Elisp.Env

namespace Elisp

/-- Evaluation errors. -/
inductive EvalError where
  | unboundVariable   : (name : Sym) → EvalError
  | unboundFunction   : (name : Sym) → EvalError
  | wrongNumberOfArgs : (expected got : Nat) → EvalError
  | wrongTypeArgument : (msg : String) → EvalError
  | thrown            : (tag value : Val) → EvalError
  | internalError     : (msg : String) → EvalError
  deriving Repr, Inhabited

abbrev EvalResult := Except EvalError (Env × Val)

/-- Parse a cons-chain into a flat list. -/
def toList : Val → List Val
  | .cons a b => a :: toList b
  | .nil      => []
  | v         => [v]

/-- Parse binding forms: ((x 1) (y 2)) → [(x, valExpr)] -/
def parseBindings : List Val → List (Sym × Val)
  | [] => []
  | .cons (.sym s) (.cons v .nil) :: rest => (s, v) :: parseBindings rest
  | .sym s :: rest => (s, .nil) :: parseBindings rest
  | _ :: rest => parseBindings rest

/-- Flatten a list of frames into a single frame. -/
def flattenFrames (frames : List Frame) : Frame :=
  frames.foldl (· ++ ·) []

/-- Parse lambda parameter list into (positional, rest). -/
def parseLambdaParams : List Val → List Sym → Bool
    → Except EvalError (List Sym × Option Sym)
  | [], acc, _                    => .ok (acc.reverse, none)
  | .sym "&rest" :: rest, acc, _  => parseLambdaParams rest acc true
  | .sym s :: _, acc, true        => .ok (acc.reverse, some s)
  | .sym s :: rest, acc, false    => parseLambdaParams rest (s :: acc) false
  | _, _, _                       => .error (.wrongTypeArgument "symbol expected in lambda list")

/-- Bind positional params to args, returning (frame, remaining_args). -/
def bindParams : List Sym → List Val → Frame → Frame × List Val
  | [], remaining, acc       => (acc.reverse, remaining)
  | p :: ps, v :: as_, acc   => bindParams ps as_ ((p, v) :: acc)
  | p :: ps, [], acc         => bindParams ps [] ((p, .nil) :: acc)

/-- The core evaluator. All helpers are defined via `where` to enable
    mutual recursion under a single `partial` annotation. -/
partial def eval (env : Env) (expr : Val) : EvalResult :=
  match expr with
  | .nil       => .ok (env, .nil)
  | .t         => .ok (env, .t)
  | .int n     => .ok (env, .int n)
  | .str s     => .ok (env, .str s)
  | .lam p r b e => .ok (env, .lam p r b e)
  | .sym s     =>
    match env.lookup s with
    | some v => .ok (env, v)
    | none   => .error (.unboundVariable s)
  | .cons car cdr =>
    let args := toList cdr
    match car with
    | .sym "quote" =>
      match args with
      | [v] => .ok (env, v)
      | _   => .error (.wrongNumberOfArgs 1 args.length)
    | .sym "if"              => evalIf env args
    | .sym "progn"           => evalProgn env args
    | .sym "setq"            => evalSetq env args
    | .sym "let"             => evalLet env args
    | .sym "let*"            => evalLetStar env args
    | .sym "dlet"            => evalDlet env args
    | .sym "lambda"          => evalLambda env args
    | .sym "defun"           => evalDefun env args
    | .sym "catch"           => evalCatch env args
    | .sym "throw"           => evalThrow env args
    | .sym "unwind-protect"  => evalUnwindProtect env args
    | _                      => evalCall env car args
where
  evalIf (env : Env) (args : List Val) : EvalResult :=
    match args with
    | cond :: then_ :: else_ => do
      let (env', condVal) ← eval env cond
      if condVal != .nil then eval env' then_
      else evalProgn env' else_
    | [cond] => do
      let (env', condVal) ← eval env cond
      if condVal != .nil then .error (.wrongNumberOfArgs 2 1)
      else .ok (env', .nil)
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalProgn (env : Env) (forms : List Val) : EvalResult :=
    match forms with
    | []        => .ok (env, .nil)
    | [f]       => eval env f
    | f :: rest => do
      let (env', _) ← eval env f
      evalProgn env' rest

  evalSetq (env : Env) (args : List Val) : EvalResult :=
    match args with
    | []  => .ok (env, .nil)
    | [_] => .error (.wrongTypeArgument "odd number of args to setq")
    | .sym s :: valExpr :: rest => do
      let (env', v) ← eval env valExpr
      let env'' := env'.setVar s v
      if rest.isEmpty then .ok (env'', v)
      else evalSetq env'' rest
    | _ => .error (.wrongTypeArgument "symbol expected in setq")

  evalLet (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bindsVal :: body =>
      let bindSpecs := parseBindings (toList bindsVal)
      evalLetBinds env bindSpecs [] body
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalLetBinds (env : Env) (specs : List (Sym × Val)) (acc : Frame)
      (body : List Val) : EvalResult :=
    match specs with
    | [] =>
      let innerEnv := { env with lex := env.lex.push acc }
      evalProgn innerEnv body
    | (s, valExpr) :: rest => do
      let (env', v) ← eval env valExpr
      evalLetBinds env' rest (acc ++ [(s, v)]) body

  evalLetStar (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bindsVal :: body =>
      let bindSpecs := parseBindings (toList bindsVal)
      evalLetStarGo env bindSpecs body
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalLetStarGo (env : Env) (specs : List (Sym × Val))
      (body : List Val) : EvalResult :=
    match specs with
    | [] => evalProgn env body
    | (s, valExpr) :: rest => do
      let (env', v) ← eval env valExpr
      evalLetStarGo { env' with lex := env'.lex.push [(s, v)] } rest body

  evalDlet (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bindsVal :: body =>
      let bindSpecs := parseBindings (toList bindsVal)
      let savedDyn := env.dyn
      evalDletGo env bindSpecs savedDyn body
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalDletGo (env : Env) (specs : List (Sym × Val)) (savedDyn : DynEnv)
      (body : List Val) : EvalResult :=
    match specs with
    | [] => do
      let (env', v) ← evalProgn env body
      .ok ({ env' with dyn := savedDyn }, v)
    | (s, valExpr) :: rest => do
      let (env', v) ← eval env valExpr
      evalDletGo { env' with dyn := env'.dyn.push [(s, v)] } rest savedDyn body

  evalLambda (env : Env) (args : List Val) : EvalResult :=
    match args with
    | paramsVal :: body =>
      let paramList := toList paramsVal
      match parseLambdaParams paramList [] false with
      | .error e => .error e
      | .ok (params, rest) =>
        let captured := flattenFrames env.lex.frames
        .ok (env, .lam params rest body captured)
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  evalDefun (env : Env) (args : List Val) : EvalResult :=
    match args with
    | .sym name :: paramsVal :: body => do
      let (env', lamVal) ← evalLambda env (paramsVal :: body)
      let env'' := { env' with functions := (name, lamVal) :: env'.functions }
      .ok (env'', .sym name)
    | _ => .error (.wrongTypeArgument "defun requires a symbol name")

  evalCatch (env : Env) (args : List Val) : EvalResult :=
    match args with
    | tagExpr :: body => do
      let (env', tag) ← eval env tagExpr
      match evalProgn env' body with
      | .ok result => .ok result
      | .error (.thrown thrownTag value) =>
        if thrownTag == tag then .ok (env', value)
        else .error (.thrown thrownTag value)
      | .error e => .error e
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  evalThrow (env : Env) (args : List Val) : EvalResult :=
    match args with
    | [tagExpr, valExpr] => do
      let (env', tag) ← eval env tagExpr
      let (_, value) ← eval env' valExpr
      .error (.thrown tag value)
    | _ => .error (.wrongNumberOfArgs 2 args.length)

  evalUnwindProtect (env : Env) (args : List Val) : EvalResult :=
    match args with
    | bodyForm :: cleanup =>
      match eval env bodyForm with
      | .ok (env', v) => do
        let (env'', _) ← evalProgn env' cleanup
        .ok (env'', v)
      | .error e =>
        let _ := evalProgn env cleanup
        .error e
    | _ => .error (.wrongNumberOfArgs 1 args.length)

  evalCall (env : Env) (fnExpr : Val) (args : List Val) : EvalResult := do
    let (env', fnVal) ← eval env fnExpr
    let (env'', evaledArgs) ← evalCallArgs env' args []
    applyFn env'' fnVal evaledArgs

  evalCallArgs (env : Env) (remaining : List Val) (acc : List Val)
      : Except EvalError (Env × List Val) :=
    match remaining with
    | [] => .ok (env, acc.reverse)
    | arg :: rest => do
      let (env', v) ← eval env arg
      evalCallArgs env' rest (v :: acc)

  applyFn (env : Env) (fn : Val) (args : List Val) : EvalResult :=
    match fn with
    | .lam params rest body captured =>
      let (frame, remaining) := bindParams params args []
      let frame' := match rest with
        | some restSym =>
          let restList := remaining.foldr (fun v a => Val.cons v a) .nil
          frame ++ [(restSym, restList)]
        | none => frame
      let closureEnv : LexEnv := { frames := [captured] }
      let callEnv := { env with lex := closureEnv.push frame' }
      match evalProgn callEnv body with
      | .ok (_, result) => .ok (env, result)
      | .error e => .error e
    | .sym s =>
      match env.lookupFn s with
      | some fnVal => applyFn env fnVal args
      | none => .error (.unboundFunction s)
    | _ => .error (.wrongTypeArgument s!"not a function: {repr fn}")

end Elisp
