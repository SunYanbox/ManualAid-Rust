## 关于XML解析器的逻辑

### 核心设计原则

解析器本质上是一个**状态机**，在文本流中逐字符扫描。其核心设计原则如下：
*   **宽容原则**：面对不合规的 XML 结构（如未闭合的标签、孤立的 CDATA），解析器不应崩溃或阻塞，而应采取“忽略噪音、保留意图”的策略。
*   **透明原则**：对于未知标签，解析器应保持透明，不干扰扫描过程。
*   **最小转义原则**：模型输出只需转义工具自身的标签，其他标签按原样保留。

### 状态机逻辑

解析器主要包含以下三种状态：
1.  **空闲状态**
    *   当前不在任何工具调用内。
    *   **触发条件**：扫描到已定义的工具开始标签（如 `<read>`）。
    *   **行为**：打开工具上下文，进入**工具内状态**。
2.  **CDATA状态**
    - 扫描**紧贴参数或工具闭合标签的**`]]>`（`]]>` 与闭合标签之间不允许任何字符，含空白）
    - 扫到`]]></参数闭合标签>`：存储CDATA中的所有文本并记录为工具参数，回到工具内状态
    - 扫到`]]></工具闭合标签>`：抛弃中途的参数并警告参数标签未闭合，结束当前工具调用（已有参数照常入队），回到空闲状态继续扫描
    - 没扫到就一直扫；不紧贴闭合标签的`]]>` 按原文继续累积
3.  **工具内状态**
    *   目标：搜索工具名闭合标签
    *   **触发条件**：
        *   扫描到已定义自身的参数开始标签（如 read工具的`<file_path>`）。
        *   扫描到当前工具的闭合标签（如 `</read>`）。
    *   **行为**：
        *   遇参数开始标签：打开参数上下文，进入**参数内状态**。
        *   遇工具闭合标签：结束当前工具调用，回到**空闲状态**。
4.  **参数内状态**
    *   当前正在捕获某个参数的值。
    *   **触发条件**：
        *   立刻扫描开始标签是否紧跟`<![CDATA[`（紧跟即紧贴，中间不允许任何字符，含空白；不是紧跟的情况，该条分支就不继续扫了，只扫下面的分支），扫描到进入CDATA状态
        *   扫描到当前参数的闭合标签（如 `</file_path>`）。则记录中间所有文本为工具参数。
    *   **行为**：保存参数值，关闭参数上下文，回到**工具内状态**。
5.  **保存工具参数状态（即时状态）**
    - 转义参数中的合法实体和数字引用，其他所有内容等原样保留。

HTML 注释（`<!-- ... -->`）一律忽略：任何状态下都跳过其内容，包括在参数标签内

未定义工具标签：忽略

非当前工具的合法参数标签：忽略

### 示例

> 此处的Json格式仅为示例，不代表解析结果的数据结构

```
<![CDATA[ <![CDATA[ <![CDATA[ // 这三个应该忽略，但当前会导致“输入中未识别到任何工具调用”
<![CDATA[...]]>  // 这里的也不应该解析
<![CDATA[ // 这样也不该解析
<edit>]]> // 这样也不该解析——忽略“]]>”
  <file_path>README.md</file_path>
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path="README.md", old_string="abcdefg,hijklmn"}`

```
<edit><![CDATA[ // 这样也不该解析——忽略
  <file_path>README.md</file_path>
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path="README.md", old_string="abcdefg,hijklmn"}`

```
<edit><![CDATA[ // 这样也不该解析——忽略
  <file_path>]]>README.md</file_path> // 这样也不该解析(即把"]]>README.md"作为原字符串保留)
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path="]]>README.md", old_string="abcdefg,hijklmn"}`

```
<edit>
  <file_path>README.md<![CDATA[</file_path>]]> // 这样也不该解析——忽略"]]>", "README.md<![CDATA["作为参数传递
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path="README.md<![CDATA[", old_string="abcdefg,hijklmn"}`

```
<edit>
  <file_path><![CDATA[README.md]]></file_path> // 这样才能解析
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path="README.md", old_string="abcdefg,hijklmn"}`

```
<edit>
  <file_path> <![CDATA[README.md]]></file_path> // 这样不能解析，因为不是紧跟合法参数标签
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path=" <![CDATA[README.md]]>", old_string="abcdefg,hijklmn"}`

```
<edit>
  <file_path><![CDATA[README.md]]> </file_path> // 这样不能解析，因为`]]>`不是紧跟合法参数标签
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit工具的file_path参数标签未正确闭合`

```
<edit>
  <file_path> <![CDATA[README.md]]> </file_path> // 这样不能解析，因为不是紧跟合法参数标签
  <old_string>abcdefg,hijklmn</old_string>
</edit>
```

应当解析为：`edit{file_path=" <![CDATA[README.md]]> ", old_string="abcdefg,hijklmn"}`


```
<edit><![CDATA[ // 这样也不该解析——忽略
  <file_path>README.md</file_path>]]>  // 这样也不该解析——忽略
  <old_string>abcdefg,hijklmn</old_string>
]]></edit>  // 这样也不该解析——忽略
```

应当解析为：`edit{file_path="README.md", old_string="abcdefg,hijklmn"}`
