//! ppa_engine_tests.rs — PPA (Pascal-like) 宏脚本引擎集成测试 (WBS-09)
//!
//! 覆盖目标:
//! 1. 算术运算与字符串拼接求值测试;
//! 2. 变量赋值与作用域隔离/继承测试;
//! 3. if/then/else 分支逻辑及关系/逻辑比较测试;
//! 4. while 与 for (to / downto) 循环累加测试;
//! 5. 自定义 procedure 与 function 声明、参数绑定与递归调用测试;
//! 6. 模拟 Sakura 编辑器 S_InsertText 与 S_GetLineStr 文本联动修改及自定义分派测试.

use editcore::ppa_engine::{
    DefaultMacroContext, MacroContext, PpaError, PpaInterpreter, PpaProgram, PpaValue,
};

#[test]
fn test_arithmetic_and_string_concat() {
    let mut interp = PpaInterpreter::new();

    // 基础算术优先级
    let code1 = "
        var
          r1, r2, r3, r4, r5, r6: Integer;
          f1: Real;
        begin
          r1 := 10 + 20 * 3;
          r2 := (10 + 20) * 3;
          r3 := 100 div 7;
          r4 := 100 mod 7;
          r5 := -15 + 30;
          r6 := $1A + $05; // 26 + 5 = 31
          f1 := 15 / 2;
        end;
    ";
    interp.eval(code1).expect("Code 1 execution failed");

    assert_eq!(interp.get_var("r1"), Some(PpaValue::Integer(70)));
    assert_eq!(interp.get_var("r2"), Some(PpaValue::Integer(90)));
    assert_eq!(interp.get_var("r3"), Some(PpaValue::Integer(14)));
    assert_eq!(interp.get_var("r4"), Some(PpaValue::Integer(2)));
    assert_eq!(interp.get_var("r5"), Some(PpaValue::Integer(15)));
    assert_eq!(interp.get_var("r6"), Some(PpaValue::Integer(31)));
    assert_eq!(interp.get_var("f1"), Some(PpaValue::Float(7.5)));

    // 字符串拼接与单引号转义
    let code2 = "
        var
          s1, s2, s3, s4: String;
        begin
          s1 := 'Hello, ' + 'World!';
          s2 := 'Number: ' + 42;
          s3 := 'Pi is approximately ' + 3.14;
          s4 := 'It''s a Pascal string';
        end;
    ";
    interp.eval(code2).expect("Code 2 execution failed");

    assert_eq!(
        interp.get_var("s1"),
        Some(PpaValue::String("Hello, World!".to_string()))
    );
    assert_eq!(
        interp.get_var("s2"),
        Some(PpaValue::String("Number: 42".to_string()))
    );
    assert_eq!(
        interp.get_var("s3"),
        Some(PpaValue::String("Pi is approximately 3.14".to_string()))
    );
    assert_eq!(
        interp.get_var("s4"),
        Some(PpaValue::String("It's a Pascal string".to_string()))
    );

    // 标准内置库函数: Length, Copy, Pos, IntToStr, StrToInt, Trim, UpperCase, LowerCase
    let code3 = "
        var
          len, p1, p2, n: Integer;
          sub1, sub2, s_int, up, low, tr: String;
        begin
          len := Length('Antigravity');
          sub1 := Copy('SakuraEditor', 1, 6);
          sub2 := Copy('SakuraEditor', 7, 6);
          p1 := Pos('ura', 'SakuraEditor');
          p2 := Pos('xyz', 'SakuraEditor');
          s_int := IntToStr(9876);
          n := StrToInt('54321');
          up := UpperCase('sakura');
          low := LowerCase('PASCAL');
          tr := Trim('   hello space   ');
        end;
    ";
    interp.eval(code3).expect("Code 3 execution failed");

    assert_eq!(interp.get_var("len"), Some(PpaValue::Integer(11)));
    assert_eq!(
        interp.get_var("sub1"),
        Some(PpaValue::String("Sakura".to_string()))
    );
    assert_eq!(
        interp.get_var("sub2"),
        Some(PpaValue::String("Editor".to_string()))
    );
    assert_eq!(interp.get_var("p1"), Some(PpaValue::Integer(4)));
    assert_eq!(interp.get_var("p2"), Some(PpaValue::Integer(0)));
    assert_eq!(
        interp.get_var("s_int"),
        Some(PpaValue::String("9876".to_string()))
    );
    assert_eq!(interp.get_var("n"), Some(PpaValue::Integer(54321)));
    assert_eq!(
        interp.get_var("up"),
        Some(PpaValue::String("SAKURA".to_string()))
    );
    assert_eq!(
        interp.get_var("low"),
        Some(PpaValue::String("pascal".to_string()))
    );
    assert_eq!(
        interp.get_var("tr"),
        Some(PpaValue::String("hello space".to_string()))
    );
}

#[test]
fn test_variables_and_scoping() {
    let mut interp = PpaInterpreter::new();

    let code = "
        var
          global_a: Integer;
          global_b: Integer;

        procedure TestScope(param_val: Integer);
        var
          local_a: Integer;
        begin
          local_a := param_val * 10;
          global_b := global_b + local_a;
          // 修改本地 shadowing 变量不会影响全局 global_a
          global_a := global_a + 1;
        end;

        begin
          global_a := 100;
          global_b := 20;
          TestScope(5);
        end.
    ";

    interp.eval(code).expect("Scope test execution failed");

    // global_a 由 100 + 1 = 101
    assert_eq!(interp.get_var("global_a"), Some(PpaValue::Integer(101)));
    // global_b 由 20 + (5 * 10) = 70
    assert_eq!(interp.get_var("global_b"), Some(PpaValue::Integer(70)));
    // local_a 在过程退出后不存在于全局作用域中
    assert_eq!(interp.get_var("local_a"), None);
}

#[test]
fn test_if_then_else() {
    let mut interp = PpaInterpreter::new();

    let code = "
        var
          x, y: Integer;
          max_val: Integer;
          branch_name: String;
          nested_res: Integer;
          bool_flag: Boolean;
        begin
          x := 15;
          y := 25;

          if x > y then
            max_val := x
          else
            max_val := y;

          if (x < 20) and (y > 20) then
            branch_name := 'in_range'
          else
            branch_name := 'out_of_range';

          bool_flag := not (x = y);

          // 嵌套 if 测试
          if x > 10 then
          begin
            if y > 30 then
              nested_res := 1
            else
              nested_res := 2;
          end
          else
            nested_res := 3;
        end;
    ";

    interp.eval(code).expect("If-then-else execution failed");

    assert_eq!(interp.get_var("max_val"), Some(PpaValue::Integer(25)));
    assert_eq!(
        interp.get_var("branch_name"),
        Some(PpaValue::String("in_range".to_string()))
    );
    assert_eq!(interp.get_var("bool_flag"), Some(PpaValue::Boolean(true)));
    assert_eq!(interp.get_var("nested_res"), Some(PpaValue::Integer(2)));
}

#[test]
fn test_while_and_for_loops() {
    let mut interp = PpaInterpreter::new();

    let code = "
        var
          sum_to: Integer;
          sum_downto: Integer;
          while_acc: Integer;
          counter: Integer;
          i: Integer;
        begin
          // For ... to
          sum_to := 0;
          for i := 1 to 10 do
            sum_to := sum_to + i;

          // For ... downto
          sum_downto := 0;
          for i := 10 downto 1 do
            sum_downto := sum_downto + i;

          // While ... do
          while_acc := 0;
          counter := 5;
          while counter > 0 do
          begin
            while_acc := while_acc + counter;
            counter := counter - 1;
          end;
        end;
    ";

    interp.eval(code).expect("Loop test execution failed");

    assert_eq!(interp.get_var("sum_to"), Some(PpaValue::Integer(55)));
    assert_eq!(interp.get_var("sum_downto"), Some(PpaValue::Integer(55)));
    assert_eq!(interp.get_var("while_acc"), Some(PpaValue::Integer(15)));
    assert_eq!(interp.get_var("counter"), Some(PpaValue::Integer(0)));
}

#[test]
fn test_custom_procedures_and_functions() {
    let mut interp = PpaInterpreter::new();

    let code = "
        program CustomRoutineDemo;

        var
          fib_res: Integer;
          fact_res: Integer;
          acc_total: Integer;

        procedure Accumulate(delta: Integer);
        begin
          acc_total := acc_total + delta;
        end;

        function Factorial(n: Integer): Integer;
        begin
          if n <= 1 then
            Result := 1
          else
            Result := n * Factorial(n - 1);
        end;

        function Fibonacci(n: Integer): Integer;
        begin
          if n <= 0 then
            Fibonacci := 0
          else if n = 1 then
            Fibonacci := 1
          else
            Fibonacci := Fibonacci(n - 1) + Fibonacci(n - 2);
        end;

        begin
          acc_total := 10;
          Accumulate(5);
          Accumulate(15);

          fact_res := Factorial(6);   // 6! = 720
          fib_res := Fibonacci(7);     // Fib(7) = 13
        end.
    ";

    interp.eval(code).expect("Routines demo execution failed");

    assert_eq!(interp.get_var("acc_total"), Some(PpaValue::Integer(30)));
    assert_eq!(interp.get_var("fact_res"), Some(PpaValue::Integer(720)));
    assert_eq!(interp.get_var("fib_res"), Some(PpaValue::Integer(13)));
}

#[test]
fn test_sakura_editor_integration() {
    let mut editor = DefaultMacroContext::new(vec![
        "Line 1: Hello World".to_string(),
        "Line 2: Sakura Editor PPA".to_string(),
        "Line 3: End of File".to_string(),
    ]);

    let mut interp = PpaInterpreter::new();

    let script = "
        var
          l1, l2: String;
          total_lines: Integer;
        begin
          // 读取行文本
          l1 := S_GetLineStr(1);
          l2 := S_GetLineStr(2);
          total_lines := S_GetLineCount();

          // 在第 1 行光标移到末尾追加
          S_MoveCursor(1, 20);
          S_InsertText(' -- Modified');

          // 在第 2 行开头插入前缀
          S_MoveCursor(2, 1);
          S_InsertText('[V2] ');

          // 在第 3 行之后插入新行
          S_MoveCursor(3, 20);
          S_InsertText('\nLine 4: Newly Inserted');
        end;
    ";

    interp
        .eval_with(script, &mut editor)
        .expect("Sakura macro execution failed");

    // 验证变量结果
    assert_eq!(
        interp.get_var("l1"),
        Some(PpaValue::String("Line 1: Hello World".to_string()))
    );
    assert_eq!(
        interp.get_var("l2"),
        Some(PpaValue::String("Line 2: Sakura Editor PPA".to_string()))
    );
    assert_eq!(interp.get_var("total_lines"), Some(PpaValue::Integer(3)));

    // 验证文本联动修改
    assert_eq!(editor.lines[0], "Line 1: Hello World -- Modified");
    assert_eq!(editor.lines[1], "[V2] Line 2: Sakura Editor PPA");
    assert_eq!(editor.lines[2], "Line 3: End of File");
    assert_eq!(editor.lines[3], "Line 4: Newly Inserted");
    assert_eq!(editor.lines.len(), 4);
}

#[test]
fn test_custom_macro_callback_dispatch() {
    let mut custom_calls = Vec::new();

    let mut interp = PpaInterpreter::new();
    interp.set_callback(move |name, args| {
        custom_calls.push((name.to_string(), args.to_vec()));
        if name.eq_ignore_ascii_case("custom_add") {
            let a = args[0].as_int()?;
            let b = args[1].as_int()?;
            Ok(PpaValue::Integer(a + b))
        } else {
            Ok(PpaValue::Void)
        }
    });

    let script = "
        var
          res: Integer;
        begin
          res := custom_add(100, 250);
          S_CustomNotification('Done');
        end;
    ";

    interp.eval(script).expect("Custom callback failed");
    assert_eq!(interp.get_var("res"), Some(PpaValue::Integer(350)));
}

struct TestEditorContext {
    inserted: Vec<String>,
}

impl MacroContext for TestEditorContext {
    fn call(&mut self, name: &str, args: &[PpaValue]) -> Result<PpaValue, PpaError> {
        if name.eq_ignore_ascii_case("S_InsertText") {
            let s = args.first().map(|v| v.to_ppa_string()).unwrap_or_default();
            self.inserted.push(s);
            Ok(PpaValue::Void)
        } else {
            Err(PpaError::UndefinedFunction(name.to_string()))
        }
    }
}

#[test]
fn test_macro_context_struct_and_program_ast() {
    let script = "
        program AstDemo;
        var
          count: Integer;
        procedure Step(s: String);
        begin
          S_InsertText(s);
        end;
        begin
          Step('alpha');
          Step('beta');
        end.
    ";

    let program = PpaProgram::parse(script).expect("Program parse failed");
    assert_eq!(program.name.as_deref(), Some("AstDemo"));
    assert_eq!(program.global_vars.len(), 1);
    assert_eq!(program.procedures.len(), 1);
    assert_eq!(program.procedures[0].name, "Step");

    let mut ctx = TestEditorContext {
        inserted: Vec::new(),
    };
    let mut interp = PpaInterpreter::new();
    interp.execute_with(&program, &mut ctx).expect("Execute AST failed");

    assert_eq!(ctx.inserted, vec!["alpha", "beta"]);
}

#[test]
fn test_error_handling() {
    let mut interp = PpaInterpreter::new();

    // 1. 除零错误
    let err_div = interp.eval("begin x := 10 div 0; end;").unwrap_err();
    assert_eq!(err_div, PpaError::DivisionByZero);

    // 2. 未定义变量
    let err_var = interp.eval("begin x := unassigned_var + 1; end;").unwrap_err();
    assert!(matches!(err_var, PpaError::UndefinedVariable(_)));

    // 3. 词法/语法错误
    let err_syntax = interp.eval("begin for i := 1 10 do; end;").unwrap_err();
    assert!(matches!(err_syntax, PpaError::ParseError { .. }));
}

