# ZDOTDIR points here so hush.zsh loads after the user's startup files, which are
# sourced with the original ZDOTDIR.
HUSH_ZDOTDIR=$ZDOTDIR
if [[ -f "$HUSH_USER_ZDOTDIR/.zshenv" ]]; then
  ZDOTDIR=$HUSH_USER_ZDOTDIR
  builtin source "$HUSH_USER_ZDOTDIR/.zshenv"
  HUSH_USER_ZDOTDIR=$ZDOTDIR
  ZDOTDIR=$HUSH_ZDOTDIR
fi
