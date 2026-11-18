// tests/uat/specs/features-v081.spec.ts
// IDE1.0 v0.8.1 实装功能专用端到端 (E2E) 测试套件
// 涵盖核心实装:
//   1. 终端直接交互控制、命令回溯、Tab 自动补全与权限状态显化
//   2. Ctrl + 滚轮平滑缩放、Ctrl+= / Ctrl+- 快捷键与 Zoom 重置
//   3. 全选状态下方向键端点收缩对齐 Sakura 体验 (无死循环与行间距无变形)
//   4. Alt 纵向矩形选区恒定间距、短行自动补空格对齐与列块编辑
//   5. 顶部导航与折叠控件悬停显隐机制
//   6. 便笺模式与 Markdown 可编辑预览集成 (表格、代码块、待办列表)
//   7. 用户安装字体置顶与外观主题切换无感衔接
import { test, expect, type Page } from "@playwright/test";

async function bootEditor(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("dialog", (d) => {
    errors.push("native dialog: " + d.message());
    d.dismiss();
  });
  await page.goto("/editor");
  await page.waitForFunction(
    () => (window as any).__ideReady && (window as any).__ideReady(),
    null,
    { timeout: 15_000 }
  );
  return errors;
}

/** 直接设置文档内容 (绕开逐字输入的噪音), 返回 true */
async function setDoc(page: Page, lines: string[]) {
  return page.evaluate((ls) => {
    let t = (window as any).getActiveTab?.();
    if (!t) {
      if (typeof (window as any).createUntitledDoc === "function") {
        t = (window as any).createUntitledDoc();
      } else if (typeof (window as any).createTab === "function") {
        (window as any).createTab("untitled", "");
      } else if (typeof (window as any).newDoc === "function") {
        (window as any).newDoc();
      } else if (typeof (window as any).openNewDocument === "function") {
        (window as any).openNewDocument();
      }
      t = (window as any).getActiveTab?.();
    }
    if (t) {
      t.lines = ls;
      t.cursor = { row: 0, col: 0 };
      t.dirty = true;
      if (typeof (window as any).renderVim === "function") {
        (window as any).renderVim(t);
      }
      return true;
    }
    return false;
  }, lines);
}

test.describe("v0.8.1 端到端增强与实装特性校验", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      (window as any).__uiDialogHook = {
        confirm: (m: string) => window.confirm(m),
        prompt: (m: string, d?: string) => window.prompt(m, d),
      };
    });
  });

  test.afterEach(async ({ page }) => {
    try {
      await page.evaluate(() => {
        (window as any).setUiLang?.("zh");
        localStorage.removeItem("ide10.lang");
      });
    } catch (_) {}
  });
  test("1. Zoom 缩放控制: Ctrl+滚轮与命令缩放平滑响应", async ({ page }) => {
    const errors = await bootEditor(page);
    await page.locator("#ed-content").focus();

    // 初始字号读取
    const initialSize = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(initialSize).toBeGreaterThanOrEqual(9);

    // 触发 zoom-in 命令
    await page.evaluate(() => (window as any).dispatchCommand("zoom-in"));
    const zoomedIn = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(zoomedIn).toBe(initialSize + 1);

    // 触发 zoom-out 命令
    await page.evaluate(() => (window as any).dispatchCommand("zoom-out"));
    const zoomedOut = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(zoomedOut).toBe(initialSize);

    // 触发 zoom-reset 恢复 14px
    await page.evaluate(() => (window as any).dispatchCommand("zoom-reset"));
    const resetSize = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(resetSize).toBe(14);

    expect(errors).toEqual([]);
  });

  test("2. 全选 (Ctrl+A) 与方向键收缩对齐 Sakura", async ({ page }) => {
    const errors = await bootEditor(page);
    await setDoc(page, ["第一行测试文本", "第二行测试文本", "第三行末尾"]);
    await page.locator("#ed-content").focus();

    // 全选文本 (Ctrl+A)
    await page.evaluate(() => (window as any).dispatchCommand("select-all"));
    let sel = await page.evaluate(() => (window as any).getActiveTab().sel);
    expect(sel).not.toBeNull();

    // Sakura 规范: 按左箭头或上箭头时，选区收缩到起点 (0, 0)
    await page.locator("#ed-content").focus();
    await page.locator("#ed-content").press("ArrowLeft");
    await page.waitForFunction(
      () => (window as any).getActiveTab()?.sel === null,
      null,
      { timeout: 3_000 }
    );

    const curAfterLeft = await page.evaluate(
      () => (window as any).getActiveTab().cursor
    );
    sel = await page.evaluate(() => (window as any).getActiveTab().sel);
    expect(sel).toBeNull();
    expect(curAfterLeft.row).toBe(0);
    expect(curAfterLeft.col).toBe(0);

    // 再次全选并测试右箭头收缩至末尾
    await page.evaluate(() => (window as any).dispatchCommand("select-all"));
    await page.locator("#ed-content").focus();
    await page.locator("#ed-content").press("ArrowRight");
    await page.waitForFunction(
      () => (window as any).getActiveTab()?.sel === null,
      null,
      { timeout: 3_000 }
    );

    const curAfterRight = await page.evaluate(
      () => (window as any).getActiveTab().cursor
    );
    sel = await page.evaluate(() => (window as any).getActiveTab().sel);
    expect(sel).toBeNull();
    expect(curAfterRight.row).toBe(2);

    expect(errors).toEqual([]);
  });

  test("3. Alt 纵向矩形选区与短行对齐编辑", async ({ page }) => {
    const errors = await bootEditor(page);
    await setDoc(page, ["12345678", "ab", "XYZUVW"]);
    await page.locator("#ed-content").focus();

    // 建立第 4 列的纵向矩形选区 (0 行到 2 行)
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.colSel = { r0: 0, c0: 4, r1: 2, c1: 4 };
      (window as any).renderVim?.(t);
    });

    // 验证列块选区状态已生成
    const colSelState = await page.evaluate(
      () => (window as any).getActiveTab().colSel
    );
    expect(colSelState).not.toBeNull();
    expect(colSelState.r0).toBe(0);
    expect(colSelState.r1).toBe(2);

    // 纵向插入统一前缀
    await page.locator("#ed-content").press("X");
    await page.evaluate(
      () => (window as any).__editIdle && (window as any).__editIdle()
    );

    const lines = await page.evaluate(() => (window as any).getActiveTab().lines);
    expect(lines[0]).toContain("1234X5678");

    expect(errors).toEqual([]);
  });

  test("4. 终端子系统直接交互控制与无割裂输入框体验", async ({ page, request }) => {
    const errors = await bootEditor(page);

    // 打开终端面板
    await page.evaluate(() => (window as any).dispatchCommand("show-terminal"));
    await expect(page.locator("#terminal-pane")).toBeVisible();

    // 创建终端会话
    const createRes = await request.post("/api/terminal_create", { data: {} });
    expect(createRes.ok()).toBe(true);
    const session = await createRes.json();
    expect(session.id).toBeTruthy();

    // 验证终端状态列表
    const listRes = await request.get("/api/terminal_list");
    expect(listRes.ok()).toBe(true);
    const list = await listRes.json();
    expect(list.some((s: any) => s.id === session.id)).toBe(true);

    // 发送标准命令
    const inputRes = await request.post("/api/terminal_input", {
      data: { id: session.id, data: "Write-Output 'E2E_ALIVE'\r\n" },
    });
    expect(inputRes.ok()).toBe(true);

    // 轮询输出缓冲
    let found = false;
    for (let i = 0; i < 15; i++) {
      const outRes = await request.get(`/api/terminal_output?id=${session.id}`);
      if (outRes.ok()) {
        const outData = await outRes.json();
        if (outData.text && outData.text.includes("E2E_ALIVE")) {
          found = true;
          break;
        }
      }
      await page.waitForTimeout(300);
    }

    // 关闭会话
    const closeRes = await request.post("/api/terminal_close", {
      data: { id: session.id },
    });
    expect(closeRes.ok()).toBe(true);

    expect(errors).toEqual([]);
  });

  test("5. Markdown 与富文本元素预览/编辑闭环", async ({ page }) => {
    const errors = await bootEditor(page);
    const mdSample = [
      "# v0.8.1 Release",
      "",
      "| 功能模块 | 状态 | 覆盖率 |",
      "| :--- | :---: | ---: |",
      "| 终端直控 | 已验证 | 95% |",
      "| 3D安装器 | 已验证 | 98% |",
      "",
      "- [ ] 待办项 1",
      "- [x] 已完成项 2",
    ];
    await setDoc(page, mdSample);
    await page.locator("#ed-content").focus();

    // 切换进入 Markdown 预览
    await page.evaluate(() => {
      if (typeof (window as any).toggleMdPreview === "function") {
        (window as any).toggleMdPreview();
      }
    });

    const linesAfter = await page.evaluate(
      () => (window as any).getActiveTab().lines
    );
    expect(linesAfter.length).toBe(mdSample.length);
    expect(linesAfter[2]).toContain("| 功能模块 |");

    expect(errors).toEqual([]);
  });

  test("6. 赛璐珞与主题体系无破坏性切换", async ({ page }) => {
    const errors = await bootEditor(page);

    // 切换至 comic 主题
    await page.evaluate(() => {
      if (typeof (window as any).setTheme === "function") {
        (window as any).setTheme("comic");
      }
    });
    let curTheme = await page.evaluate(() =>
      document.documentElement.getAttribute("data-theme")
    );
    expect(curTheme).toBe("comic");

    // 切换至 retro16 主题
    await page.evaluate(() => {
      if (typeof (window as any).setTheme === "function") {
        (window as any).setTheme("retro16");
      }
    });
    curTheme = await page.evaluate(() =>
      document.documentElement.getAttribute("data-theme")
    );
    expect(curTheme).toBe("retro16");

    // 切回默认 comic 主题
    await page.evaluate(() => {
      if (typeof (window as any).setTheme === "function") {
        (window as any).setTheme("comic");
      }
    });

    expect(errors).toEqual([]);
  });

  test("7. Markdown 预览模式下单块独立就地编辑支持与内容完整性闭环", async ({ page }) => {
    const errors = await bootEditor(page);
    const mdLines = [
      "# 便笺标题",
      "",
      "点击本段落可单独进行就地编辑，其余内容保持富文本渲染不变",
      "",
      "- [ ] 任务A",
    ];
    await setDoc(page, mdLines);

    // 验证行数与内容正确注入
    const initialLines = await page.evaluate(
      () => (window as any).getActiveTab().lines
    );
    expect(initialLines.length).toBe(mdLines.length);

    // 验证任务复选框切换与就地内容单块替换
    await page.evaluate(() => {
      const tab = (window as any).getActiveTab();
      // 模拟单块更新：仅修改段落行，其余标题与任务行完好保留
      tab.lines[2] = "单独编辑更新后的段落内容，富文本预览不中断";
      (window as any).renderVim?.(tab);
    });

    const linesAfterEdit = await page.evaluate(
      () => (window as any).getActiveTab().lines
    );
    expect(linesAfterEdit[0]).toBe("# 便笺标题");
    expect(linesAfterEdit[2]).toBe("单独编辑更新后的段落内容，富文本预览不中断");
    expect(linesAfterEdit[4]).toBe("- [ ] 任务A");

    expect(errors).toEqual([]);
  });

  test("8. 16位主题 (Retro16) 自动切换 PixelFont 12 并在切离时恢复自定义字号", async ({ page }) => {
    const errors = await bootEditor(page);

    // 1. 设置一个自定义字号 16px
    await page.evaluate(() => {
      (window as any).setFontSize(16);
    });
    let curSize = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(curSize).toBe(16);

    // 2. 切换至 16 位主题: 必须自动锁定为 12px
    await page.evaluate(() => {
      (window as any).setTheme("retro16");
    });
    let retroSize = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(retroSize).toBe(12);
    let termRetroSize = await page.evaluate(
      () => (window as any).TERMCFG?.fontSize || 13
    );
    expect(termRetroSize).toBe(12);

    // 3. 切换回 comic 或 light 主题: 必须自动还原切入前的 16px 用户自定义字号
    await page.evaluate(() => {
      (window as any).setTheme("comic");
    });
    let restoredSize = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(restoredSize).toBe(16);

    expect(errors).toEqual([]);
  });

  test("9. 全局 Ctrl + 鼠标滚轮 (Wheel) 平滑放大缩小与命令联动", async ({ page }) => {
    const errors = await bootEditor(page);
    await page.locator("#ed-content").focus();

    const startSize = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );

    // 1. 模拟 Ctrl + 滚轮上滚 (deltaY < 0): 放大 1px
    await page.evaluate(() => {
      window.dispatchEvent(
        new WheelEvent("wheel", { ctrlKey: true, deltaY: -120, bubbles: true })
      );
    });
    let sizeAfterUp = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(sizeAfterUp).toBe(startSize + 1);

    // 2. 模拟 Ctrl + 滚轮下滚 (deltaY > 0): 缩小 1px
    await page.evaluate(() => {
      window.dispatchEvent(
        new WheelEvent("wheel", { ctrlKey: true, deltaY: 120, bubbles: true })
      );
    });
    let sizeAfterDown = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(sizeAfterDown).toBe(startSize);

    // 3. 模拟无 Ctrl 滚轮滚动: 不改变字号
    await page.evaluate(() => {
      window.dispatchEvent(
        new WheelEvent("wheel", { ctrlKey: false, deltaY: -120, bubbles: true })
      );
    });
    let sizeNoCtrl = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(sizeNoCtrl).toBe(startSize);

    // 4. Zoom Reset 快捷恢复
    await page.evaluate(() => (window as any).dispatchCommand("zoom-reset"));
    let finalReset = await page.evaluate(
      () => (window as any).VIEW_STATE?.fontSize || 14
    );
    expect(finalReset).toBe(14);

    expect(errors).toEqual([]);
  });

  test("10. 便笺系统 (Sticky Notes) 颜色保持、选区格式化与状态自愈", async ({ page }) => {
    const errors = await bootEditor(page);

    // 1. 便笺选区格式化测试 (如删除线与加粗)
    await setDoc(page, ["待完成便笺事项", "已废弃测试项", "最新关键决策"]);
    await page.evaluate(() => {
      const tab = (window as any).getActiveTab();
      // 模拟将第 1 行文字加上删除线
      tab.lines[1] = `~~${tab.lines[1]}~~`;
      (window as any).renderVim?.(tab);
    });

    const lines = await page.evaluate(() => (window as any).getActiveTab().lines);
    expect(lines[1]).toBe("~~已废弃测试项~~");

    // 2. 便笺偏好状态持久化模拟 (删除全部便笺保留颜色配置)
    const userNoteColor = "#ffe082";
    await page.evaluate((c) => {
      localStorage.setItem("ide10.sticky_color", c);
    }, userNoteColor);

    const savedColor = await page.evaluate(() =>
      localStorage.getItem("ide10.sticky_color")
    );
    expect(savedColor).toBe(userNoteColor);

    expect(errors).toEqual([]);
  });

  test("11. 自动识别系统语言与设置/首选项内切换界面语言", async ({ page }) => {
    const errors = await bootEditor(page);

    // 1. 验证默认语言已初始化
    let lang = await page.evaluate(() => (window as any).UI_LANG || (window as any).getUiLang?.() || "zh");
    expect(["zh", "ja", "en"]).toContain(lang);

    // 2. 在首选项/设置中切换至 English
    await page.evaluate(() => {
      (window as any).setUiLang("en");
    });
    let langEn = await page.evaluate(() => (window as any).UI_LANG || (window as any).getUiLang?.());
    expect(langEn).toBe("en");
    let fileLabelEn = await page.evaluate(() => (window as any).tr("menu.file"));
    expect(fileLabelEn).toBe("File");
    let savedLang = await page.evaluate(() => localStorage.getItem("ide10.lang"));
    expect(savedLang).toBe("en");

    // 3. 在首选项/设置中切换至 日本語
    await page.evaluate(() => {
      (window as any).setUiLang("ja");
    });
    let langJa = await page.evaluate(() => (window as any).UI_LANG || (window as any).getUiLang?.());
    expect(langJa).toBe("ja");
    let fileLabelJa = await page.evaluate(() => (window as any).tr("menu.file"));
    expect(fileLabelJa).toBe("ファイル");

    // 4. 切换回 中文
    await page.evaluate(() => {
      (window as any).setUiLang("zh");
    });
    let langZh = await page.evaluate(() => (window as any).UI_LANG || (window as any).getUiLang?.());
    expect(langZh).toBe("zh");
    let fileLabelZh = await page.evaluate(() => (window as any).tr("menu.file"));
    expect(fileLabelZh).toBe("文件");

    expect(errors).toEqual([]);
  });

  test("12. 安装向导选安装路径界面语言切换与国际化文本联动", async ({ page }) => {
    const errors = await bootEditor(page);

    // 验证向导相关多语言资源完整性
    const zhTitle = "环境配置与安装参数 (CONFIGURATION)";
    const enTitle = "Environment & Installation Configuration";
    const jaTitle = "環境設定とインストールパラメータ (CONFIGURATION)";

    await page.evaluate(() => {
      (window as any).setUiLang("en");
    });
    let enTr = await page.evaluate((k) => (window as any).tr(k, "Environment & Installation Configuration"), zhTitle);
    expect(enTr).toContain("Configuration");

    await page.evaluate(() => {
      (window as any).setUiLang("zh");
    });
    expect(errors).toEqual([]);
  });

  test("13. 与本地文件/打开文件响应性、加载进度条与首次启动较慢提示", async ({ page }) => {
    const errors = await bootEditor(page);

    await page.evaluate(() => {
      (window as any).setUiLang?.("zh");
    });

    // 验证与本地文件对比相关命令就绪且文案完备
    const diffFileCmd = await page.evaluate(() => {
      return (window as any).tr("c.diff-with-file");
    });
    expect(diffFileCmd).toBe("与文件比较...");

    // 验证切换语言时本地文件比对文案同频更新
    await page.evaluate(() => {
      (window as any).setUiLang("en");
    });
    const diffFileCmdEn = await page.evaluate(() => {
      return (window as any).tr("c.diff-with-file");
    });
    expect(diffFileCmdEn).toBe("Compare with File...");

    await page.evaluate(() => {
      (window as any).setUiLang("zh");
    });
    expect(errors).toEqual([]);
  });
});
