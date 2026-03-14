#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSTEST_DIR="${SCRIPT_DIR}/systest"
SYSTEST_TXT="${SCRIPT_DIR}/systest.txt"
SYSDEPS_TXT="${SCRIPT_DIR}/sysdeps.txt"
TIMEOUT_SEC="160"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

show_help() {
    cat << 'EOF'
系统测试脚本

用法:
  ./systest.sh -a                                    # 测试所有 crates.io 上的包
  ./systest.sh -l                                    # 测试所有本地包
  ./systest.sh -l ch2 sbi                            # 测试本地 ch2, sbi
  ./systest.sh -l ch2 sbi -r ch5 ch6                 # 混合：本地 ch2,sbi + 远程 ch5,ch6
  ./systest.sh ch2 ch3                               # 测试远程 ch2, ch3（默认远程）

选项:
  -a          测试所有包，使用 crates.io 依赖（全远程模式）
  -l          指定后续包名使用本地依赖（直到遇到 -r）
  -r          指定后续包名使用 crates.io 依赖（直到遇到 -l）
  -h, --help  显示此帮助信息

配置文件:
  systest.txt   测试目标定义
  sysdeps.txt   本地依赖定义

包名简写:
  ch1, ch2, ... ch8  等价于 tg-rcore-tutorial-ch1, ...
  sbi                等价于 tg-rcore-tutorial-sbi
  (其他同理)
EOF
    exit 0
}

expand_crate_name() {
    local name="$1"
    case "$name" in
        ch1|ch2|ch3|ch4|ch5|ch6|ch7|ch8)
            echo "tg-rcore-tutorial-$name"
            ;;
        sbi|syscall|console|linker|checker|user|sync|signal|signal-defs|signal-impl|easy-fs|kernel-alloc|kernel-context|kernel-vm|task-manage)
            echo "tg-rcore-tutorial-$name"
            ;;
        tg-rcore-tutorial-*)
            echo "$name"
            ;;
        *)
            echo "$name"
            ;;
    esac
}

for arg in "$@"; do
    if [[ "$arg" == "-h" || "$arg" == "--help" ]]; then
        show_help
    fi
done

if [[ ! -f "${SYSTEST_TXT}" ]]; then
    echo -e "${RED}错误：找不到 ${SYSTEST_TXT}${NC}" >&2
    exit 1
fi

declare -A SYSTEST_CMDS
declare -A SYSTEST_EXPECTED
declare -a ALL_CRATE_NAMES

while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    line=$(echo "$line" | tr -d '\r')
    
    fields=()
    temp="$line"
    while [[ "$temp" =~ \"([^\"]*)\" ]]; do
        fields+=("${BASH_REMATCH[1]}")
        temp="${temp#*\"}"
        temp="${temp#*\"}"
    done
    
    if [[ ${#fields[@]} -ge 3 ]]; then
        crate="${fields[0]}"
        ALL_CRATE_NAMES+=("$crate")
        SYSTEST_CMDS["$crate"]="${fields[1]}"
        SYSTEST_EXPECTED["$crate"]="${fields[2]}"
    fi
done < "${SYSTEST_TXT}"

declare -A LOCAL_DEP_PATHS

if [[ -f "${SYSDEPS_TXT}" ]]; then
    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        line=$(echo "$line" | tr -d '\r')
        
        fields=()
        temp="$line"
        while [[ "$temp" =~ \"([^\"]*)\" ]]; do
            fields+=("${BASH_REMATCH[1]}")
            temp="${temp#*\"}"
            temp="${temp#*\"}"
        done
        
        if [[ ${#fields[@]} -ge 2 ]]; then
            LOCAL_DEP_PATHS["${fields[0]}"]="${fields[1]}"
        fi
    done < "${SYSDEPS_TXT}"
fi

ALL_REMOTE_MODE=false
declare -A CRATE_USE_LOCAL

current_mode="remote"

for arg in "$@"; do
    case "$arg" in
        -a)
            ALL_REMOTE_MODE=true
            ;;
        -l)
            current_mode="local"
            ;;
        -r)
            current_mode="remote"
            ;;
        *)
            expanded=$(expand_crate_name "$arg")
            CRATE_USE_LOCAL["$expanded"]=$([[ "$current_mode" == "local" ]] && echo "true" || echo "false")
            ;;
    esac
done

if [[ "$ALL_REMOTE_MODE" == "true" ]]; then
    for crate in "${ALL_CRATE_NAMES[@]}"; do
        CRATE_USE_LOCAL["$crate"]=false
    done
elif [[ ${#CRATE_USE_LOCAL[@]} -eq 0 ]]; then
    for crate in "${ALL_CRATE_NAMES[@]}"; do
        CRATE_USE_LOCAL["$crate"]=$([[ "$current_mode" == "local" ]] && echo "true" || echo "false")
    done
fi

declare -a TEST_CRATES
for crate in "${ALL_CRATE_NAMES[@]}"; do
    if [[ -n "${CRATE_USE_LOCAL[$crate]+x}" ]]; then
        TEST_CRATES+=("$crate")
    fi
done

if [[ ${#TEST_CRATES[@]} -eq 0 ]]; then
    echo -e "${RED}错误：没有指定要测试的 crate${NC}" >&2
    exit 1
fi

echo "========================================"
echo "  tg-rcore-tutorial-sbi 系统测试"
echo "========================================"
echo "测试目录:      ${SYSTEST_DIR}"
echo "超时限制:      ${TIMEOUT_SEC} 秒"
echo ""

echo "本次测试 crate (${#TEST_CRATES[@]} 个):"
for crate in "${TEST_CRATES[@]}"; do
    if [[ "${CRATE_USE_LOCAL[$crate]}" == "true" ]]; then
        echo -e "  ${CYAN}[本地]${NC} ${crate}"
    else
        echo -e "  [远程] ${crate}"
    fi
done
echo ""

local_dep_count=${#LOCAL_DEP_PATHS[@]}
if [[ $local_dep_count -gt 0 ]]; then
    echo "本地依赖配置 ($local_dep_count 个):"
    for dep in "${!LOCAL_DEP_PATHS[@]}"; do
        echo "  - $dep → ${LOCAL_DEP_PATHS[$dep]}"
    done
    echo ""
fi

if ! command -v cargo-clone &>/dev/null && ! cargo clone --version &>/dev/null 2>&1; then
    echo "cargo-clone 未安装，正在安装..."
    cargo install cargo-clone
fi

mkdir -p "${SYSTEST_DIR}"

echo "========================================"
echo "阶段1：获取各章节 crate 源码"
echo "========================================"

for CRATE in "${TEST_CRATES[@]}"; do
    TARGET_DIR="${SYSTEST_DIR}/${CRATE}"
    
    if [[ -d "${TARGET_DIR}" ]]; then
        echo "  [清理] 删除旧目录 ${TARGET_DIR}"
        rm -rf "${TARGET_DIR}"
    fi
    
    if [[ "${CRATE_USE_LOCAL[$CRATE]}" == "true" ]]; then
        LOCAL_SRC_PATH="${LOCAL_DEP_PATHS[$CRATE]:-}"
        
        if [[ -z "$LOCAL_SRC_PATH" ]]; then
            echo -e "  ${RED}[错误]${NC} 本地源码路径 ${CRATE} 未在 sysdeps.txt 中定义"
            exit 1
        fi
        
        ABS_LOCAL_SRC="${SCRIPT_DIR}/${LOCAL_SRC_PATH}"
        
        if [[ ! -d "${ABS_LOCAL_SRC}" ]]; then
            echo -e "  ${RED}[错误]${NC} 本地源码目录不存在: ${ABS_LOCAL_SRC}"
            exit 1
        fi
        
        echo "  [拷贝] 从 ${ABS_LOCAL_SRC} 拷贝到 ${TARGET_DIR}"
        cp -r "${ABS_LOCAL_SRC}" "${TARGET_DIR}"
        echo "  [完成] ${CRATE} 拷贝完成"
    else
        echo "  [下载] 正在 cargo clone ${CRATE} ..."
        (cd "${SYSTEST_DIR}" && cargo clone "${CRATE}")
        echo "  [完成] ${CRATE} 下载完成"
    fi
done
echo ""

declare -a LOCAL_MODE_CRATES
for crate in "${TEST_CRATES[@]}"; do
    if [[ "${CRATE_USE_LOCAL[$crate]}" == "true" ]]; then
        LOCAL_MODE_CRATES+=("$crate")
    fi
done

if [[ ${#LOCAL_MODE_CRATES[@]} -gt 0 && ${#LOCAL_DEP_PATHS[@]} -gt 0 ]]; then
    echo "========================================"
    echo "阶段2：patch Cargo.toml 使用本地依赖"
    echo "========================================"

    DEP_LIST="["
    first=true
    for dep_name in "${!LOCAL_DEP_PATHS[@]}"; do
        if [[ "$first" != "true" ]]; then DEP_LIST+=","; fi
        DEP_LIST+="('${dep_name}', '${LOCAL_DEP_PATHS[$dep_name]}')"
        first=false
    done
    DEP_LIST+="]"

    for CRATE in "${LOCAL_MODE_CRATES[@]}"; do
        TARGET_DIR="${SYSTEST_DIR}/${CRATE}"
        CARGO_TOML="${TARGET_DIR}/Cargo.toml"

        if [[ ! -f "${CARGO_TOML}" ]]; then
            echo "  [警告] ${CRATE}/Cargo.toml 不存在，跳过"
            continue
        fi

        echo "  [patch] ${CRATE}/Cargo.toml"
        cp "${CARGO_TOML}" "${CARGO_TOML}.bak"

        python3 - <<PYEOF
import re

cargo_toml = '${CARGO_TOML}'
dep_list = ${DEP_LIST}

with open(cargo_toml, 'r') as f:
    content = f.read()

for pkg_name, rel_path in dep_list:
    content = re.sub(
        r'^(' + re.escape(pkg_name) + r'\s*=\s*)"[^"]*"',
        r'\1{ path = "' + rel_path + r'" }',
        content, flags=re.MULTILINE
    )

    def make_replace_table(path):
        def replace_table(m):
            prefix = m.group(1)
            inner  = m.group(2)
            inner  = re.sub(r'\bversion\s*=\s*"[^"]*",?\s*', '', inner)
            inner  = inner.strip().rstrip(',').strip()
            body   = ('path = "' + path + '", ' + inner) if inner else ('path = "' + path + '"')
            return prefix + '{ ' + body + ' }'
        return replace_table

    content = re.sub(
        r'^(' + re.escape(pkg_name) + r'\s*=\s*)\{([^}]*)\}',
        make_replace_table(rel_path), content, flags=re.MULTILINE
    )

    def make_replace_dep_table(path, name):
        def replace_dep_table(m):
            header = m.group(1)
            body = m.group(2)
            if 'package = "' + name + '"' in body:
                body = re.sub(r'^version\s*=\s*"[^"]*"\s*\n', '', body, flags=re.MULTILINE)
                return header + 'path = "' + path + '"\n' + body
            return m.group(0)
        return replace_dep_table

    content = re.sub(
        r'(\[dependencies\.[^\]]+\]\n)(.*?)(?=\n\[|\Z)',
        make_replace_dep_table(rel_path, pkg_name), content, flags=re.DOTALL
    )

with open(cargo_toml, 'w') as f:
    f.write(content)
print('    已写入 ' + cargo_toml)
PYEOF
    done
    echo ""
fi

echo "========================================"
echo "阶段3：执行测试"
echo "========================================"

declare -a PASSED_LIST
declare -a FAILED_LIST
declare -a TIMEOUT_LIST

 TOTAL="${#TEST_CRATES[@]}"
IDX=0

for CRATE in "${TEST_CRATES[@]}"; do
    TEST_CMD="${SYSTEST_CMDS[$CRATE]}"
    EXPECTED="${SYSTEST_EXPECTED[$CRATE]}"
    TARGET_DIR="${SYSTEST_DIR}/${CRATE}"
    IDX=$((IDX + 1))

    mode_str=$([[ "${CRATE_USE_LOCAL[$CRATE]}" == "true" ]] && echo "[本地]" || echo "[远程]")

    echo ""
    echo "----------------------------------------"
    echo "[${IDX}/${TOTAL}] ${mode_str} 测试 ${CRATE}"
    echo "  期望输出: ${EXPECTED}"
    echo "  命令:     cd ${TARGET_DIR} && timeout ${TIMEOUT_SEC} ${TEST_CMD}"
    echo "----------------------------------------"

    if [[ ! -d "${TARGET_DIR}" ]]; then
        echo -e "  ${RED}[错误]${NC} 目录不存在，跳过"
        FAILED_LIST+=("${CRATE} (目录不存在)")
        continue
    fi

    TEST_SCRIPT="${TARGET_DIR}/test.sh"
    if [[ ! -f "${TEST_SCRIPT}" ]]; then
        echo -e "  ${RED}[错误]${NC} test.sh 不存在，跳过"
        FAILED_LIST+=("${CRATE} (test.sh 不存在)")
        continue
    fi

    LOG_FILE="${SYSTEST_DIR}/${CRATE}.log"
    rm -f "${LOG_FILE}"

    echo "  ---- test.sh 输出 ----"
    set +e
    (cd "${TARGET_DIR}" && setsid script -q -f -e -c "timeout ${TIMEOUT_SEC} ${TEST_CMD}" "${LOG_FILE}" </dev/null >/dev/null 2>/dev/null) &
    SCRIPT_PID=$!
    while [[ ! -s "${LOG_FILE}" ]] && kill -0 "${SCRIPT_PID}" 2>/dev/null; do sleep 0.1; done
    if [[ -s "${LOG_FILE}" ]]; then
        tail -f "${LOG_FILE}" 2>/dev/null &
        TAIL_PID=$!
    fi
    wait "${SCRIPT_PID}"
    EXIT_CODE=$?
    [[ -n "${TAIL_PID:-}" ]] && kill "${TAIL_PID}" 2>/dev/null
    set -e
    echo ""
    echo "  ---- 输出结束 (exit=${EXIT_CODE}) ----"

    if [[ $EXIT_CODE -eq 124 ]]; then
        echo -e "  ${YELLOW}[超时]${NC} ${TIMEOUT_SEC} 秒内未完成"
        TIMEOUT_LIST+=("${CRATE}")
        continue
    fi

    ACTUAL_LINE=$(sed $'s/\x1b\[[0-9;]*[a-zA-Z]//g' "${LOG_FILE}" 2>/dev/null | grep "Test PASSED:" | tail -1 | tr -d '\r' | xargs || true)
    EXPECTED_TRIMMED=$(echo "${EXPECTED}" | xargs)
    echo "  实际输出: ${ACTUAL_LINE:-（未找到 'Test PASSED:' 行）}"

    if [[ -z "$ACTUAL_LINE" ]]; then
        echo -e "  ${RED}[失败]${NC} 未找到 'Test PASSED:' 行 (退出码=${EXIT_CODE})"
        FAILED_LIST+=("${CRATE} (无 Test PASSED: 行,exit=${EXIT_CODE})")
    elif [[ "$ACTUAL_LINE" == *"$EXPECTED_TRIMMED"* ]]; then
        echo -e "  ${GREEN}[通过]${NC} 输出与期望一致"
        PASSED_LIST+=("${CRATE}")
    else
        echo -e "  ${RED}[失败]${NC} 输出与期望不一致"
        echo "    期望包含: ${EXPECTED_TRIMMED}"
        echo "    实际输出: ${ACTUAL_LINE}"
        FAILED_LIST+=("${CRATE} (期望='${EXPECTED_TRIMMED}')")
    fi
done

echo ""
echo "========================================"
echo "  测试汇总报告"
echo "========================================"
echo ""

echo -e "${GREEN}=== 正常运行 (${#PASSED_LIST[@]}/${TOTAL}) ===${NC}"
if [[ ${#PASSED_LIST[@]} -eq 0 ]]; then
    echo "  （无）"
else
    for item in "${PASSED_LIST[@]}"; do echo "  [PASS] ${item}"; done
fi

echo ""
echo -e "${RED}=== 运行失败 (${#FAILED_LIST[@]}/${TOTAL}) ===${NC}"
if [[ ${#FAILED_LIST[@]} -eq 0 ]]; then
    echo "  （无）"
else
    for item in "${FAILED_LIST[@]}"; do echo "  [FAIL] ${item}"; done
fi

echo ""
echo -e "${YELLOW}=== 超时未完成 (${#TIMEOUT_LIST[@]}/${TOTAL}) ===${NC}"
if [[ ${#TIMEOUT_LIST[@]} -eq 0 ]]; then
    echo "  （无）"
else
    for item in "${TIMEOUT_LIST[@]}"; do echo "  [TIMEOUT] ${item} (>${TIMEOUT_SEC}s)"; done
fi

echo ""
echo "========================================"
echo "  日志目录: ${SYSTEST_DIR}/"
echo "========================================"

if [[ ${#FAILED_LIST[@]} -eq 0 && ${#TIMEOUT_LIST[@]} -eq 0 ]]; then
    echo -e "${GREEN}所有测试均通过！${NC}"
    exit 0
else
    echo -e "${RED}存在失败或超时测试，请检查上述报告。${NC}"
    exit 1
fi
