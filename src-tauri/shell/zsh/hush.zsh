# Hush shell integration for zsh.
#
#   OSC 133;A / 133;B   prompt start / end (Hush edits the command after B)
#   OSC 133;C           command started
#   OSC 133;D;<code>    command finished
#   OSC 7               working directory (file:// URL)
#   OSC 7777;key=value  history file, PATH, aliases and functions
#
# \e[9001~ and \e[9002~ run or cancel the command Hush wrote to the command file.

[[ -o interactive ]] || return 0
(( ${+__hush_loaded} )) && return 0
typeset -g __hush_loaded=1
typeset -gi __hush_running=0
typeset -g __hush_reported_path= __hush_reported_names=

__hush_osc() {
  builtin printf '\e]%s\a' "$1"
}

__hush_report_cwd() {
  emulate -L zsh
  # Percent-encode everything outside the RFC 3986 unreserved set.
  local LC_ALL=C dir=$PWD encoded= char hex
  local -i i
  for (( i = 1; i <= ${#dir}; i++ )); do
    char=${dir[i]}
    if [[ $char == [A-Za-z0-9/._~-] ]]; then
      encoded+=$char
    else
      builtin printf -v hex '%%%02X' "'$char"
      encoded+=$hex
    fi
  done
  __hush_osc "7;file://${HOST}${encoded}"
}

__hush_report_meta() {
  if [[ $PATH != $__hush_reported_path ]]; then
    __hush_reported_path=$PATH
    __hush_osc "7777;path=$PATH"
  fi
  # Skip _/- prefixed functions (completion internals).
  local names="${(j: :)${(@k)aliases}} ${(j: :)${(@)${(@k)functions}:#[_-]*}}"
  if [[ $names != $__hush_reported_names ]]; then
    __hush_reported_names=$names
    __hush_osc "7777;names=$names"
  fi
}

__hush_precmd() {
  local -i code=$?
  if (( __hush_running )); then
    __hush_osc "133;D;$code"
    __hush_running=0
  fi
  __hush_report_cwd
  __hush_report_meta
  # Prompt themes may rebuild PS1 on every prompt, so re-wrap when needed.
  if [[ $PS1 != *$'\e]133;A'* ]]; then
    PS1=$'%{\e]133;A\a%}'"$PS1"$'%{\e]133;B\a%}'
  fi
}

__hush_preexec() {
  __hush_running=1
  __hush_osc "133;C"
}

# Loading BUFFER from a file (instead of replaying keystrokes) runs the command as
# written: tabs, newlines and paste-quoting plugins can't alter it.
typeset -g __hush_command_file=$HUSH_COMMAND_FILE
unset HUSH_COMMAND_FILE

__hush_load_command() {
  BUFFER=$(<"$__hush_command_file")
  CURSOR=${#BUFFER}
  # Same colors Hush used, so the redraw on accept-line doesn't flicker.
  local highlights=${__hush_command_file:r}.hl
  region_highlight=()
  [[ -s $highlights ]] && region_highlight=("${(@f)$(<$highlights)}")
}
__hush_run() {
  __hush_load_command
  zle accept-line
}
__hush_cancel() {
  __hush_load_command
  zle redisplay
  zle send-break
}
zle -N __hush_run
zle -N __hush_cancel
() {
  local keymap
  for keymap in emacs viins vicmd; do
    bindkey -M $keymap '\e[9001~' __hush_run
    bindkey -M $keymap '\e[9002~' __hush_cancel
  done
}

__hush_osc "7777;histfile=${HISTFILE:-${HUSH_USER_ZDOTDIR:-$HOME}/.zsh_history}"

autoload -Uz add-zsh-hook
add-zsh-hook precmd __hush_precmd
add-zsh-hook preexec __hush_preexec
