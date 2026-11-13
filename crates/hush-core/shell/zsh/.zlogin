# Last startup file: restore the user's ZDOTDIR for subshells.
ZDOTDIR=$HUSH_USER_ZDOTDIR
if [[ -f "$ZDOTDIR/.zlogin" ]]; then
  builtin source "$ZDOTDIR/.zlogin"
fi
unset HUSH_ZDOTDIR HUSH_USER_ZDOTDIR
