" vim:sw=4:

" Misc
set splitright
set splitbelow
set ff=unix " set file format to unix style (line ending <LF> instead of <CR><LF>)
set ttyfast " indicates fast terminal connection

" Spelling
setlocal spell
set spelllang=en_gb

" Set relative line numbers
set relativenumber
set number

" Set cursor line
set cursorline

" Set default encoding
set encoding=UTF-8

" Set tab size
set tabstop=4
set softtabstop=4
set noexpandtab " always use tabs
set shiftwidth=4
set autoindent
set shiftround

" Enable mouse support
set mouse=a

" Set colorcolumn
set colorcolumn=80

" Load plugins
call plug#begin(stdpath('data') . '/plugged')
{{!--
    Only install fzf on windows, on linux it will be installed via the package
    manager.
    The PUNKTF_TARGET_OS variable is automatically set by punktf.
--}}
{{@if {{$PUNKTF_TARGET_OS}} == "windows"}}
    Plug 'junegunn/fzf', { 'do': { -> fzf#install() } }
{{@fi}}
    Plug 'junegunn/fzf.vim'
call plug#end()
