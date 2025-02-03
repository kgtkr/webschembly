pass=0
failure=0
for list in ${@}; do
    echo "Testing $list.scm"
    diff -r $list.snapshot $list.result
    if [ $? -ne 0 ]; then
        failure=$((failure+1))
        echo "\033[0;31mFailed: $list\033[0m"
    else
        pass=$((pass+1))
        echo "\033[0;32mPassed: $list\033[0m"
    fi
done
echo "==="
if [ $failure -ne 0 ]; then
    echo "\033[0;32mPassed: $pass\033[0m"
    echo "\033[0;31mFailed: $failure\033[0m"
    exit 1
else
    echo "\033[0;32mAll tests passed\033[0m"
fi
