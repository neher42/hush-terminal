# /etc/zshrc derived HISTFILE from our ZDOTDIR; point it back at the user's.
if [[ $HISTFILE == "$HUSH_ZDOTDIR/.zsh_history" ]]; then
  HISTFILE=$HUSH_USER_ZDOTDIR/.zsh_history
fi

if [[ -f "$HUSH_USER_ZDOTDIR/.zshrc" ]]; then
  ZDOTDIR=$HUSH_USER_ZDOTDIR
  builtin source "$HUSH_USER_ZDOTDIR/.zshrc"
  HUSH_USER_ZDOTDIR=$ZDOTDIR
  ZDOTDIR=$HUSH_ZDOTDIR
fi
builtin source "$HUSH_ZDOTDIR/hush.zsh"
