#!/usr/bin/env bash

set -eo pipefail

cd $(dirname $0)

stop_on_failure=""
diff=""
patch=""
filter=""
no_confirmation=""
quiet=""
keep_database=""

total=0;
failures=();

PGQL_HOST=${PGQL_HOST:-127.0.0.1}
PGQL_PORT=${PGQL_PORT:-8080}

PGQL_DB_HOST=${PGQL_DB_HOST:-db}
PGQL_DB_USER=${PGQL_DB_USER:-pgql}
PGQL_DB_PASSWORD=${PGQL_DB_PASSWORD:-$PGQL_DB_USER}
PGQL_DB_NAME=${PGQL_DB_NAME:-pgql_test}

PGQL_DB_URL=${PGQL_DB_URL:-"host=$PGQL_DB_HOST user=$PGQL_DB_USER password=$PGQL_DB_PASSWORD dbname=$PGQL_DB_NAME"}

main () {
    read_args "$@"

    reset_database

    start_server

    set +e

    run_tests
}

start_server() {
    PGQL_HOST="$PGQL_HOST" \
    PGQL_PORT="$PGQL_PORT" \
    PGQL_DB_URL="$PGQL_DB_URL" \
        ../target/debug/pgql > ./logs.txt &

    trap 'kill %1' EXIT

    timeout 3 bash -c "until echo > /dev/tcp/localhost/$PGQL_PORT; do sleep 0.1; done" 2> /dev/null
}

PGQL_ENDPOINT=http://$PGQL_HOST:$PGQL_PORT

run_introspection_expected_graphql_test () {
    check "$test" "$test" "$(npx get-graphql-schema $PGQL_ENDPOINT)"
}

run_graphql_request_test () {
    test=$1
    type=$2

    output=$( \
        jq -n --arg $type "$(<./$test)" -f ./request.jq \
            | curl -s -d@- $PGQL_ENDPOINT --header "content-type: application/json" \
            | jq . \
    )

    test_expected_output_file=$(echo "$test" | sed 's/\.test\.gql$/.expected.json/')

    check "$test" "$test_expected_output_file" "$output"
}

run_query_gql_test () {
    run_graphql_request_test "$1" query
}

run_mutation_test () {
    run_graphql_request_test "$1" mutation
    run_sql_test $(echo "$test" | sed 's/\.test\.gql$/.sql/')
}

check () {
    test=$1
    test_expected_output_file=$2
    output=$3

    total=$((total+1))

    expected_diff=$(compare_output "$test_expected_output_file" "$test" "$output")

    if [ ! -z "$expected_diff" ]; then
        failures+=( "$test" )

        if [ ! -z "$diff" ]; then
            echo
            echo "Output of test $test does not match content of $test_expected_output_file:"
            echo

            compare_output "$test_expected_output_file" "$test" "$output" --color=always
        fi

        if [ ! -z "$patch" ]; then
            reply="y"

            if [ -z "$no_confirmation" ]; then
                echo
                read -p "Apply patch to $test_expected_output_file ? y|[n] " -n 1 -r reply
                echo
            fi

            if [[ $reply =~ ^[Yy]$ ]]
            then
                echo "Updating $test_expected_output_file to match actual output"
                patch $test_expected_output_file <(echo "$expected_diff")
            fi

        fi

        if [ ! -z "$stop_on_failure" ]; then
            summary
        fi
    fi
}

compare_output () {
    test_expected_output_file=$1; shift
    test=$1; shift
    output=$1; shift
    diff \
        -Naur \
        --label "$test_expected_output_file" \
        --label "test: $test" \
        "$test_expected_output_file" \
        <(echo "$output") \
        "$@"
}

run_tests () {
    for test in **$filter**.test*; do
        executor=$(echo $test | sed 's/\(.*\.\|\)\([^.]*\)\.test\.\(.*\)/\2_\3/' | sed 's/\./_/g')
        "run_${executor}_test" $test
    done

    summary
}

summary () {
    if [ -z "$quiet" ]; then
        echo "Tests executed: $total"
        echo

        if [ ${#failures[@]} -eq 0 ]; then
            echo "All tests are successful !"
        else
            echo "Tests KO: ${#failures[@]}"

            for test in "${failures[@]}"; do
                echo "  💣 $test"
            done
        fi
    fi

    exit ${#failures[@]}
}

read_args() {
    OPTIONS=hsdpf:nkq
    LONGOPTS=help,stop-on-failure,diff,patch,filter:,no-confirmation,keep-database,quiet

    # -regarding ! and PIPESTATUS see above
    # -temporarily store output to be able to check for errors
    # -activate quoting/enhanced mode (e.g. by writing out “--options”)
    # -pass arguments only via   -- "$@"   to separate them correctly
    ! PARSED=$(getopt --options=$OPTIONS --longoptions=$LONGOPTS --name "$0" -- "$@")
    if [[ ${PIPESTATUS[0]} -ne 0 ]]; then
        # e.g. return value is 1
        #  then getopt has complained about wrong arguments to stdout
        exit 254
    fi


    # read getopt’s output this way to handle the quoting right:
    eval set -- "$PARSED"

    # now enjoy the options in order and nicely split until we see --
    while true; do
        case "$1" in
            -h|--help)
                echo "Usage: $0 [OPTIONS]"
                echo
                echo "Options:"
                echo "   -h, --help               Display this message"
                echo "   -s, --stop-on-failure    Stop at first test failure"
                echo "   -d, --diff               Show differences from expected output when not matching"
                echo "   -p, --patch              Patch expected output files to match actual output when different"
                echo "   -n, --no-confirmation    Don’t ask for confirmation before updated expected output files"
                echo "   -k, --keep-database      Don’t reset the database at start"
                echo "   -f, --filter FILTER      Only execute test matching FILTER"
                exit 0
                ;;
            -s|--stop-on-failure)
                stop_on_failure=1
                shift
                ;;
            -p|--patch)
                patch=1
                shift
                ;;
            -d|--diff)
                diff=1
                shift
                ;;
            -n|--no-confirmation)
                no_confirmation=1
                shift
                ;;
            -k|--keep-database)
                keep_database=1
                shift
                ;;
            -q|--quiet)
                quiet=1
                shift
                ;;
            -f|--filter)
                filter="$2"
                shift 2
                ;;
            --)
                shift
                break
                ;;
            *)
                echo "$0: unrecognized option '$1'"
                exit 254
                ;;
        esac
    done
}

reset_database () {
    if [ -z "$keep_database" ]; then
        psql="PGPASSWORD=\"$PGQL_DB_PASSWORD\" psql -h \"$PGQL_DB_HOST\" -U \"$PGQL_DB_USER\" -w -v ON_ERROR_STOP=1 -q"
        echo "drop database if exists $PGQL_DB_NAME; create database $PGQL_DB_NAME" | eval "$psql"
        {
            echo "begin;"
            cat ./schema.sql ./data.sql
            echo "commit;"
        } | envsubst | eval "$psql" -d "$PGQL_DB_NAME"
    fi
}

export PGQL_DB_NAME

main "$@"
