@echo off
setlocal EnableExtensions
rem Private root CA restricted to one parent domain, and a server certificate signed by it.
rem Windows companion of scripts\gen-certs.sh. Requires OpenSSL on PATH, or Git for Windows, or OpenSSL-Win64.
rem
rem   certs\forever.cert.pem         leaf + CA chain  -> bnetserver
rem   certs\forever.key.pem          server private key -> bnetserver, NEVER share
rem   certs\ca\forever_root_ca.crt   CA certificate -> players and the launcher, public
rem   certs\ca\forever_root_ca.key   CA private key -> NEVER share, NEVER commit
rem
rem HOSTNAME is the login (LoginREST) hostname your client connects to (CN and SAN).
rem The CA nameConstraints permit only CA_DOMAIN and its subdomains. CA_DOMAIN defaults to the parent
rem domain of HOSTNAME (everything after the first dot); set it in the environment to override.
rem
rem Usage:
rem   scripts\generate-certs.bat <hostname> [days]
rem   scripts\generate-certs.bat <days> <hostname>

rem Git for Windows openssl rewrites arguments that start with /. Stop that.
set "MSYS2_ARG_CONV_EXCL=*"
set "MSYS_NO_PATHCONV=1"

set "HOST="
set "DAYS=825"
if not "%~1"=="" (
    echo(%~1| findstr /r "^[0-9][0-9]*$" >nul
    if not errorlevel 1 (
        set "DAYS=%~1"
        if not "%~2"=="" set "HOST=%~2"
    ) else (
        set "HOST=%~1"
        if not "%~2"=="" set "DAYS=%~2"
    )
)

if not defined HOST (
    echo usage: %~nx0 ^<hostname^> [days]   ^(the login hostname your client connects to^)
    exit /b 1
)

echo(%DAYS%| findstr /r "^[1-9][0-9]*$" >nul
if errorlevel 1 (
    echo days must be a positive integer
    exit /b 1
)
echo(%HOST%| findstr /r "^[A-Za-z0-9.-][A-Za-z0-9.-]*$" >nul
if errorlevel 1 (
    echo hostname must be a DNS name ^(letters, digits, dots, hyphens^)
    exit /b 1
)
echo(%HOST%| findstr /c:".." >nul && (
    echo hostname must not contain ".."
    exit /b 1
)
if not defined CA_DOMAIN for /f "tokens=1,* delims=." %%a in ("%HOST%") do set "CA_DOMAIN=%%b"
if not defined CA_DOMAIN (
    echo hostname must contain a dot, or set CA_DOMAIN
    exit /b 1
)
set "HOST_OK="
if /I "%HOST%"=="%CA_DOMAIN%" set "HOST_OK=1"
echo(%HOST%| findstr /i /e /c:".%CA_DOMAIN%" >nul && set "HOST_OK=1"
if not defined HOST_OK (
    echo hostname must be %CA_DOMAIN% or a name under it ^(CA nameConstraints^).
    echo Set CA_DOMAIN before the CA is created if you need another parent domain.
    exit /b 1
)

set "OPENSSL="
for /f "delims=" %%O in ('where openssl 2^>nul') do (
    if not defined OPENSSL set "OPENSSL=%%O"
)
if not defined OPENSSL if exist "%ProgramFiles%\Git\usr\bin\openssl.exe" set "OPENSSL=%ProgramFiles%\Git\usr\bin\openssl.exe"
if not defined OPENSSL if exist "%ProgramFiles%\OpenSSL-Win64\bin\openssl.exe" set "OPENSSL=%ProgramFiles%\OpenSSL-Win64\bin\openssl.exe"
if not defined OPENSSL if exist "%ProgramFiles%\OpenSSL\bin\openssl.exe" set "OPENSSL=%ProgramFiles%\OpenSSL\bin\openssl.exe"
if not defined OPENSSL if exist "%ProgramFiles(x86)%\OpenSSL-Win64\bin\openssl.exe" set "OPENSSL=%ProgramFiles(x86)%\OpenSSL-Win64\bin\openssl.exe"
if not defined OPENSSL if exist "%ProgramFiles(x86)%\OpenSSL-Win32\bin\openssl.exe" set "OPENSSL=%ProgramFiles(x86)%\OpenSSL-Win32\bin\openssl.exe"
if not defined OPENSSL if exist "%ProgramFiles(x86)%\Git\usr\bin\openssl.exe" set "OPENSSL=%ProgramFiles(x86)%\Git\usr\bin\openssl.exe"
if not defined OPENSSL (
    echo openssl was not found.
    echo Install OpenSSL and reopen the shell, or install Git for Windows or OpenSSL-Win64.
    echo Looked on PATH and under Program Files\Git\usr\bin and Program Files\OpenSSL-Win64\bin.
    exit /b 1
)

for %%I in ("%~dp0..") do set "ROOT=%%~fI"
set "DIR=%ROOT%\certs"
set "CA=%DIR%\ca"
if not exist "%CA%" mkdir "%CA%"
if errorlevel 1 exit /b 1

if not exist "%CA%\forever_root_ca.key" (
    echo == new root CA ^(permitted DNS: %CA_DOMAIN%^)
    "%OPENSSL%" req -x509 -newkey rsa:4096 -sha256 -nodes -days 3650 -keyout "%CA%\forever_root_ca.key" -out "%CA%\forever_root_ca.crt" -subj "/CN=Forever Private Root CA" -addext "basicConstraints=critical,CA:TRUE,pathlen:0" -addext "keyUsage=critical,keyCertSign,cRLSign" -addext "nameConstraints=critical,permitted;DNS:%CA_DOMAIN%"
    if errorlevel 1 exit /b 1
) else (
    echo == reusing existing root CA %CA%\forever_root_ca.crt
)

echo == server certificate for %HOST% ^(%DAYS% days^)
"%OPENSSL%" req -newkey rsa:2048 -sha256 -nodes -keyout "%DIR%\forever.key.pem" -out "%CA%\server.csr" -subj "/CN=%HOST%"
if errorlevel 1 exit /b 1

> "%CA%\server.ext" (
    echo basicConstraints=critical,CA:FALSE
    echo keyUsage=critical,digitalSignature,keyEncipherment
    echo extendedKeyUsage=serverAuth
    echo subjectAltName=DNS:%HOST%,DNS:*.%CA_DOMAIN%
)

"%OPENSSL%" x509 -req -in "%CA%\server.csr" -CA "%CA%\forever_root_ca.crt" -CAkey "%CA%\forever_root_ca.key" -CAcreateserial -days %DAYS% -sha256 -extfile "%CA%\server.ext" -out "%CA%\server.crt"
if errorlevel 1 exit /b 1

copy /b /y "%CA%\server.crt"+"%CA%\forever_root_ca.crt" "%DIR%\forever.cert.pem" >nul
if errorlevel 1 exit /b 1
del /q "%CA%\server.csr" "%CA%\server.ext" 2>nul

icacls "%DIR%\forever.key.pem" /inheritance:r /grant:r "%USERNAME%:(R,W)" >nul 2>&1
icacls "%CA%\forever_root_ca.key" /inheritance:r /grant:r "%USERNAME%:(R,W)" >nul 2>&1

"%OPENSSL%" verify -CAfile "%CA%\forever_root_ca.crt" "%CA%\server.crt"
if errorlevel 1 exit /b 1

echo.
echo Done.
echo Copy onto the bnetserver runtime ^(not a build folder that CMake POST_BUILD may overwrite^):
echo   %DIR%\forever.cert.pem  -^>  bnetserver.cert.pem
echo   %DIR%\forever.key.pem   -^>  bnetserver.key.pem
echo Give players ONLY:
echo   %CA%\forever_root_ca.crt
echo NEVER share or commit:
echo   %CA%\forever_root_ca.key
echo Point the launcher at this CA: replace app\src-tauri\resources\forever_root_ca.crt
echo with that .crt, or set FOREVER_CA_CERT to its PEM text, then rebuild.
echo See docs\OPERATOR.md.
exit /b 0
