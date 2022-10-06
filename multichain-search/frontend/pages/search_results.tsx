import type { GetServerSideProps, NextPage } from 'next'
import { Search } from '../components/Search'

import { useRouter } from 'next/router'
import styles from '../styles/search.module.css'
import { useEffect, useState } from 'react'

import loadingIcon from 'icons/spinner.svg';
import { Icon, Spinner } from '@chakra-ui/react'
import { ProxySearchResults } from '../components/proxySearchResults'


interface Props {
  q: string
}

const SearchResults: NextPage<Props> = ({q}) => {
  const [isLoading, setLoading] = useState(false);
  const [data, setData] = useState(null)
  useEffect(() => {
    setLoading(true)
    fetch('http://localhost:8044/api/v1/search?q=' + q)
      .then((res) => res.json())
      .then((data) => {
        setData(data)
        setLoading(false)
      })
  }, [q])

  return (
    <>
        <Search initialSearchValue={q}>
          <div className={styles.output}>
            <div className={styles.output_results}>
              <p>Search results:</p>
            </div>
            
            { isLoading ? <Spinner size='md' color="blue"/> : <ProxySearchResults responses={data || {}}></ProxySearchResults> }
          </div>
        </Search>
    </>
  )
}

export const getServerSideProps: GetServerSideProps = async ({ query }) => {
  if (!query.q) {
    return {
      notFound: true,
    };
  }

  return {
    props: {q: query.q},
  };
}

export default SearchResults
